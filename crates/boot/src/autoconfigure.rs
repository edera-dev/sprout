use alloc::boxed::Box;
use anyhow::{Context, Result};
use edera_sprout_config::RootConfiguration;
use log::warn;
use uefi::Handle;
use uefi::fs::FileSystem;
use uefi::proto::device_path::DevicePath;
use uefi::proto::loaded_image::LoadedImage;
use uefi::proto::media::fs::SimpleFileSystem;

/// bls: autodetect and configure BLS-enabled filesystems.
pub mod bls;

/// linux: autodetect and configure Linux kernels.
/// This autoconfiguration module should not be activated
/// on BLS-enabled filesystems as it may make duplicate entries.
pub mod linux;

/// windows: autodetect and configure Windows boot configurations.
pub mod windows;

/// Find the filesystems that systemd-boot reads BLS entries from: the partition that Sprout
/// was loaded from, and the extended boot loader partition of the same disk.
fn strict_bls_sources(loaded_image_path: &DevicePath) -> (Option<Handle>, Option<Handle>) {
    let own = eficore::handle::open_shared::<LoadedImage>(uefi::boot::image_handle())
        .ok()
        .and_then(|image| image.device());
    let xbootldr = match eficore::xbootldr::find_xbootldr(loaded_image_path) {
        Ok(Some(path)) => uefi::boot::locate_device_path::<SimpleFileSystem>(&mut &*path).ok(),
        Ok(None) => None,
        Err(error) => {
            warn!("unable to find the xbootldr partition: {:#}", error);
            None
        }
    };
    (own, xbootldr)
}

/// Generate a [RootConfiguration] based on the environment.
/// Intakes a `config` to use as the basis of the autoconfiguration.
/// With `strict`, only the partition Sprout was loaded from and the extended boot loader
/// partition of its disk are read for BLS entries, as one generator, like systemd-boot does.
pub fn autoconfigure(
    config: &mut RootConfiguration,
    strict: bool,
    loaded_image_path: &DevicePath,
) -> Result<()> {
    // Find all the filesystems that are on the system.
    let filesystem_handles =
        uefi::boot::find_handles::<SimpleFileSystem>().context("unable to scan filesystems")?;

    // The partitions that BLS entries are read from in strict mode.
    let (own_handle, xbootldr_handle) = if strict {
        strict_bls_sources(loaded_image_path)
    } else {
        (None, None)
    };
    // The device path of the partition of Sprout, which names the strict generator.
    let mut own_root: Option<Box<DevicePath>> = None;
    // Whether either of the partitions has BLS entries, in strict mode.
    let mut strict_found = false;

    // For each filesystem that was detected, scan it for supported autoconfig mechanisms.
    for handle in filesystem_handles {
        // Acquire the device path root for the filesystem.
        // A filesystem that can't be opened is skipped, as it should not prevent
        // autoconfiguring the other filesystems.
        let root = match uefi::boot::open_protocol_exclusive::<DevicePath>(handle) {
            Ok(root) => root.to_boxed(),
            Err(error) => {
                warn!("skipping filesystem, unable to get its root: {}", error);
                continue;
            }
        };

        // Open the filesystem that was detected.
        let filesystem = match uefi::boot::open_protocol_exclusive::<SimpleFileSystem>(handle) {
            Ok(filesystem) => filesystem,
            Err(error) => {
                warn!("skipping filesystem, unable to open it: {}", error);
                continue;
            }
        };

        // Trade the filesystem protocol for the uefi filesystem helper.
        let mut filesystem = FileSystem::new(filesystem);

        // A scan that fails is skipped, as a filesystem that can't be read should not
        // prevent autoconfiguring the other filesystems.

        // Scan the filesystem for BLS supported configurations. In strict mode, only the
        // partitions that systemd-boot reads get entries, and they get a single generator.
        // The other filesystems that have BLS are still not scanned for Linux configurations.
        let is_source = Some(handle) == own_handle || Some(handle) == xbootldr_handle;
        if strict && Some(handle) == own_handle {
            own_root = Some(root.to_boxed());
        }
        let bls_found = if strict {
            match bls::detect(&mut filesystem) {
                Ok(found) => {
                    strict_found |= found && is_source;
                    found
                }
                Err(error) => {
                    warn!(
                        "unable to scan filesystem for bls configurations: {:#}",
                        error
                    );
                    false
                }
            }
        } else {
            match bls::scan(&mut filesystem, &root, config) {
                Ok(found) => found,
                Err(error) => {
                    warn!(
                        "unable to scan filesystem for bls configurations: {:#}",
                        error
                    );
                    false
                }
            }
        };

        // If BLS was not found, scan for Linux configurations.
        if !bls_found && let Err(error) = linux::scan(&mut filesystem, &root, config) {
            warn!(
                "unable to scan filesystem for linux configurations: {:#}",
                error
            );
        }

        // Always look for Windows configurations.
        if let Err(error) = windows::scan(&mut filesystem, &root, config) {
            warn!(
                "unable to scan filesystem for windows configurations: {:#}",
                error
            );
        }
    }

    // In strict mode, one generator reads both partitions, with the root of Sprout's partition.
    if strict && strict_found {
        match own_root {
            Some(root) => {
                if let Err(error) = bls::add_generator(config, &root, true) {
                    warn!("unable to add the bls generator: {:#}", error);
                }
            }
            None => warn!(
                "found BLS entries, but not the partition that Sprout was loaded from, \
                 so none are used in strict mode"
            ),
        }
    }

    Ok(())
}

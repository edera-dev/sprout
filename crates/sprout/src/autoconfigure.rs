use crate::config::RootConfiguration;
use anyhow::{Context, Result};
use uefi::fs::FileSystem;
use uefi::proto::device_path::DevicePath;
use uefi::proto::media::fs::SimpleFileSystem;

/// bls: autodetect and configure BLS-enabled filesystems.
pub mod bls;

/// linux: autodetect and configure Linux kernels.
/// This autoconfiguration module should not be activated
/// on BLS-enabled filesystems as it may make duplicate entries.
pub mod linux;

/// windows: autodetect and configure Windows boot configurations.
pub mod windows;

/// Generate a [RootConfiguration] based on the environment.
/// Intakes a `config` to use as the basis of the autoconfiguration.
pub fn autoconfigure(config: &mut RootConfiguration) -> Result<()> {
    // Find all the filesystems that are on the system.
    let filesystem_handles =
        uefi::boot::find_handles::<SimpleFileSystem>().context("unable to scan filesystems")?;

    // For each filesystem that was detected, scan it for supported autoconfig mechanisms.
    for handle in filesystem_handles {
        // Acquire the device path root for the filesystem.
        let root = {
            uefi::boot::open_protocol_exclusive::<DevicePath>(handle)
                .context("unable to get root for filesystem")?
                .to_boxed()
        };

        // Open the filesystem that was detected.
        let filesystem = uefi::boot::open_protocol_exclusive::<SimpleFileSystem>(handle)
            .context("unable to open filesystem")?;

        // Trade the filesystem protocol for the uefi filesystem helper.
        let mut filesystem = FileSystem::new(filesystem);

        // Scan the filesystem for BLS supported configurations.
        let bls_found = bls::scan(&mut filesystem, &root, config)
            .context("unable to scan for bls configurations")?;

        // If BLS was not found, scan for Linux configurations.
        if !bls_found {
            linux::scan(&mut filesystem, &root, config)
                .context("unable to scan for linux configurations")?;
        }

        // Always look for Windows configurations.
        windows::scan(&mut filesystem, &root, config)
            .context("unable to scan for windows configurations")?;
    }

    Ok(())
}

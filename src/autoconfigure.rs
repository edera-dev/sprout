use crate::actions::ActionDeclaration;
use crate::actions::chainload::ChainloadConfiguration;
use crate::config::RootConfiguration;
use crate::entries::EntryDeclaration;
use crate::generators::GeneratorDeclaration;
use crate::generators::bls::BlsConfiguration;
use anyhow::{Context, Result};
use uefi::cstr16;
use uefi::fs::{FileSystem, Path};
use uefi::proto::device_path::DevicePath;
use uefi::proto::device_path::text::{AllowShortcuts, DisplayOnly};
use uefi::proto::media::fs::SimpleFileSystem;

/// The name of the BLS chainload action that will be used
/// by the BLS generator to chainload entries.
const BLS_CHAINLOAD_ACTION: &str = "bls-chainload";

/// Scan the specified `filesystem` for BLS configurations.
fn scan_for_bls(
    filesystem: &mut FileSystem,
    root: &DevicePath,
    config: &mut RootConfiguration,
) -> Result<bool> {
    // BLS has a loader.conf file that can specify its own auto-entries mechanism.
    let bls_loader_conf_path = Path::new(cstr16!("\\loader\\loader.conf"));
    // BLS also has an entries directory that can specify explicit entries.
    let bls_entries_path = Path::new(cstr16!("\\loader\\entries"));

    // Convert the device path root to a string we can use in the configuration.
    let mut root = root
        .to_string(DisplayOnly(false), AllowShortcuts(false))
        .context("unable to convert device root to string")?
        .to_string();
    // Add a trailing slash to the root to ensure the path is valid.
    root.push('/');

    // Whether we have a loader.conf file.
    let has_loader_conf = filesystem
        .try_exists(bls_loader_conf_path)
        .context("unable to check for BLS loader.conf file")?;

    // Whether we have an entries directory.
    // We actually iterate the entries to see if there are any.
    let has_entries_dir = filesystem
        .read_dir(bls_entries_path)
        .ok()
        .and_then(|mut iterator| iterator.next())
        .map(|entry| entry.is_ok())
        .unwrap_or(false);

    // Detect if a BLS supported configuration is on this filesystem.
    // We check both loader.conf and entries directory as only one of them is required.
    if !(has_loader_conf || has_entries_dir) {
        return Ok(false);
    }

    // BLS is now detected, generate a configuration for it.
    let generator = BlsConfiguration {
        entry: EntryDeclaration {
            title: "$title".to_string(),
            actions: vec![BLS_CHAINLOAD_ACTION.to_string()],
            ..Default::default()
        },
        path: format!("{}\\loader", root),
    };

    // Generate a unique name for the BLS generator and insert the generator into the configuration.
    config.generators.insert(
        format!("autoconfigure-bls-{}", root),
        GeneratorDeclaration {
            bls: Some(generator),
            ..Default::default()
        },
    );

    // We had a BLS supported configuration, so return true.
    Ok(true)
}

/// Add the BLS actions to the configuration.
/// We should only do this once.
fn add_bls_actions(config: &mut RootConfiguration) -> Result<()> {
    // Generate a chainload configuration for BLS.
    // BLS will provide these values to us.
    let chainload = ChainloadConfiguration {
        path: "$chainload".to_string(),
        options: vec!["$options".to_string()],
        linux_initrd: Some("$initrd".to_string()),
    };

    // Insert the chainload action into the configuration.
    config.actions.insert(
        BLS_CHAINLOAD_ACTION.to_string(),
        ActionDeclaration {
            chainload: Some(chainload),
            ..Default::default()
        },
    );

    Ok(())
}

/// Generate a [RootConfiguration] based on the environment.
/// Intakes a `config` to use as the basis of the autoconfiguration.
pub fn autoconfigure(config: &mut RootConfiguration) -> Result<()> {
    // Find all the filesystems that are on the system.
    let filesystem_handles =
        uefi::boot::find_handles::<SimpleFileSystem>().context("unable to scan filesystems")?;

    // Whether we have any BLS-supported filesystems.
    // We will key off of this to know if we should add BLS actions.
    let mut has_any_bls = false;

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
        // If we find any, we will add a BLS generator to the configuration, and then
        // at the end we will add the BLS actions to the configuration.
        let bls_found =
            scan_for_bls(&mut filesystem, &root, config).context("unable to scan filesystem")?;
        has_any_bls = has_any_bls || bls_found;
    }

    // If we had any BLS-supported filesystems, add the BLS actions to the configuration.
    if has_any_bls {
        add_bls_actions(config).context("unable to add BLS actions")?;
    }

    Ok(())
}

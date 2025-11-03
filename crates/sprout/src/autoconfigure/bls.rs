use crate::actions::ActionDeclaration;
use crate::actions::chainload::ChainloadConfiguration;
use crate::config::RootConfiguration;
use crate::entries::EntryDeclaration;
use crate::generators::GeneratorDeclaration;
use crate::generators::bls::BlsConfiguration;
use crate::utils;
use anyhow::{Context, Result};
use uefi::cstr16;
use uefi::fs::{FileSystem, Path};
use uefi::proto::device_path::DevicePath;
use uefi::proto::device_path::text::{AllowShortcuts, DisplayOnly};

/// The name prefix of the BLS chainload action that will be used
/// by the BLS generator to chainload entries.
const BLS_CHAINLOAD_ACTION_PREFIX: &str = "bls-chainload-";

/// Scan the specified `filesystem` for BLS configurations.
pub fn scan(
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
    // Add a trailing forward-slash to the root to ensure the device root is completed.
    root.push('/');

    // Generate a unique hash of the root path.
    let root_unique_hash = utils::unique_hash(&root);

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

    // Generate a unique name for the BLS chainload action.
    let chainload_action_name = format!("{}{}", BLS_CHAINLOAD_ACTION_PREFIX, root_unique_hash,);

    // BLS is now detected, generate a configuration for it.
    let generator = BlsConfiguration {
        entry: EntryDeclaration {
            title: "$title".to_string(),
            actions: vec![chainload_action_name.clone()],
            ..Default::default()
        },
        path: format!("{}\\loader", root),
    };

    // Generate a unique name for the BLS generator and insert the generator into the configuration.
    config.generators.insert(
        format!("auto-bls-{}", root_unique_hash),
        GeneratorDeclaration {
            bls: Some(generator),
            ..Default::default()
        },
    );

    // Generate a chainload configuration for BLS.
    // BLS will provide these values to us.
    let chainload = ChainloadConfiguration {
        path: format!("{}\\$chainload", root),
        options: vec!["$options".to_string()],
        linux_initrd: Some(format!("{}\\$initrd", root)),
    };

    // Insert the chainload action into the configuration.
    config.actions.insert(
        chainload_action_name,
        ActionDeclaration {
            chainload: Some(chainload),
            ..Default::default()
        },
    );

    // We had a BLS supported configuration, so return true.
    Ok(true)
}

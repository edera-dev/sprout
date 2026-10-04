use crate::generators::bls::BLS_INITRD_SLOTS;
use alloc::string::ToString;
use alloc::{format, vec};
use anyhow::{Context, Result};
use edera_sprout_config::RootConfiguration;
use edera_sprout_config::actions::ActionDeclaration;
use edera_sprout_config::actions::chainload::ChainloadConfiguration;
use edera_sprout_config::entries::EntryDeclaration;
use edera_sprout_config::generators::GeneratorDeclaration;
use edera_sprout_config::generators::bls::BlsConfiguration;
use edera_sprout_parsing::unique_hash;
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
    // BLS Type #2 entries are unified kernel images in the EFI/Linux directory.
    let bls_uki_path = Path::new(cstr16!("\\EFI\\Linux"));

    // Convert the device path root to a string we can use in the configuration.
    let mut root = root
        .to_string16(DisplayOnly(false), AllowShortcuts(false))
        .context("unable to convert device root to string")?
        .to_string();
    // Add a trailing forward-slash to the root to ensure the device root is completed.
    root.push('/');

    // Generate a unique hash of the root path.
    let root_unique_hash = unique_hash(&root);

    // Whether we have a loader.conf file.
    let has_loader_conf = filesystem
        .try_exists(bls_loader_conf_path)
        .context("unable to check for BLS loader.conf file")?;

    // Whether we have an entries directory.
    // We actually iterate the entries to see if there are any entry files, as the
    // directory itself always has the "." and ".." items, even when it is empty.
    let has_entries_dir = filesystem
        .read_dir(bls_entries_path)
        .map(|mut iterator| {
            iterator.any(|entry| {
                entry.is_ok_and(|entry| {
                    entry.is_regular_file()
                        && entry
                            .file_name()
                            .to_string()
                            .to_lowercase()
                            .ends_with(".conf")
                })
            })
        })
        .unwrap_or(false);

    // Whether we have any unified kernel images.
    let has_ukis = filesystem
        .read_dir(bls_uki_path)
        .map(|mut iterator| {
            iterator.any(|entry| {
                entry.is_ok_and(|entry| {
                    entry.is_regular_file()
                        && entry
                            .file_name()
                            .to_string()
                            .to_lowercase()
                            .ends_with(".efi")
                })
            })
        })
        .unwrap_or(false);

    // Detect if a BLS supported configuration is on this filesystem.
    // We check loader.conf, the entries directory and unified kernel images, as only one of
    // them is required.
    if !(has_loader_conf || has_entries_dir || has_ukis) {
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
        uki_path: None,
        pin_names: true,
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
    // Every initrd slot is chained in order. Unused slots stamp to the root of the
    // filesystem, which the chainload action skips.
    let chainload = ChainloadConfiguration {
        path: format!("{}\\$chainload", root),
        options: vec!["$options".to_string()],
        linux_initrd: None,
        linux_initrd_chain: (0..BLS_INITRD_SLOTS)
            .map(|slot| format!("{}\\$initrd-{}", root, slot))
            .collect(),
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

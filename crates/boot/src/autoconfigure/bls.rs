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

/// Detect whether the `filesystem` has a BLS supported configuration.
pub fn detect(filesystem: &mut FileSystem) -> Result<bool> {
    // BLS has a loader.conf file that can specify its own auto-entries mechanism.
    let bls_loader_conf_path = Path::new(cstr16!("\\loader\\loader.conf"));
    // BLS also has an entries directory that can specify explicit entries.
    let bls_entries_path = Path::new(cstr16!("\\loader\\entries"));
    // BLS Type #2 entries are unified kernel images in the EFI/Linux directory.
    let bls_uki_path = Path::new(cstr16!("\\EFI\\Linux"));

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
    Ok(has_loader_conf || has_entries_dir || has_ukis)
}

/// Add the BLS generator and its action for the partition with the device path `root`.
/// In `strict` mode, the generator also reads the extended boot loader partition of the disk.
pub fn add_generator(
    config: &mut RootConfiguration,
    root: &DevicePath,
    strict: bool,
) -> Result<()> {
    // Convert the device path root to a string we can use in the configuration.
    let mut root = root
        .to_string16(DisplayOnly(false), AllowShortcuts(false))
        .context("unable to convert device root to string")?
        .to_string();
    // Add a trailing forward-slash to the root to ensure the device root is completed.
    root.push('/');

    // Generate a unique hash of the root path.
    let root_unique_hash = unique_hash(&root);

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
        // Every filesystem is scanned, including the Extended Boot Loader Partition, which
        // gets a generator of its own, unless strict mode reads it with the generator of the
        // partition that Sprout was loaded from.
        xbootldr: strict,
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
    // The files of an entry are on the partition of the entry, which is the root of the
    // generator for most entries, and the extended boot loader partition for some in strict mode.
    let entry_root = if strict { "$entry-root" } else { &root };
    let chainload = ChainloadConfiguration {
        path: format!("{}\\$chainload", entry_root),
        options: vec!["$options".to_string()],
        linux_initrd: None,
        linux_initrd_chain: (0..BLS_INITRD_SLOTS)
            .map(|slot| format!("{}\\$initrd-{}", entry_root, slot))
            .collect(),
        // An unset devicetree stamps to the root of the filesystem, which the action skips.
        devicetree: Some(format!("{}\\$devicetree", entry_root)),
    };

    // Insert the chainload action into the configuration.
    config.actions.insert(
        chainload_action_name,
        ActionDeclaration {
            chainload: Some(chainload),
            ..Default::default()
        },
    );

    Ok(())
}

/// Scan the specified `filesystem` for BLS configurations.
pub fn scan(
    filesystem: &mut FileSystem,
    root: &DevicePath,
    config: &mut RootConfiguration,
) -> Result<bool> {
    if !detect(filesystem)? {
        return Ok(false);
    }
    add_generator(config, root, false)?;
    Ok(true)
}

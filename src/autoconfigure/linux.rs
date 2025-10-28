use crate::actions::ActionDeclaration;
use crate::actions::chainload::ChainloadConfiguration;
use crate::config::RootConfiguration;
use crate::entries::EntryDeclaration;
use crate::generators::GeneratorDeclaration;
use crate::generators::list::ListConfiguration;
use crate::utils;
use anyhow::{Context, Result};
use log::info;
use std::collections::BTreeMap;
use uefi::CString16;
use uefi::fs::{FileSystem, Path};
use uefi::proto::device_path::DevicePath;
use uefi::proto::device_path::text::{AllowShortcuts, DisplayOnly};

/// The name prefix of the Linux chainload action that will be used to boot Linux.
const LINUX_CHAINLOAD_ACTION_PREFIX: &str = "linux-chainload-";

/// The locations to scan for kernel pairs.
/// We will check for symlinks and if this directory is a symlink, we will skip it.
const SCAN_LOCATIONS: &[&str] = &["/boot", "/"];

/// Prefixes of kernel files to scan for.
const KERNEL_PREFIXES: &[&str] = &["vmlinuz"];

/// Prefixes of initramfs files to match to.
const INITRAMFS_PREFIXES: &[&str] = &["initramfs", "initrd", "initrd.img"];

/// Pair of kernel and initramfs.
/// This is what scanning a directory is meant to find.
struct KernelPair {
    /// The path to a kernel.
    kernel: String,
    /// The path to an initramfs, if any.
    initramfs: Option<String>,
}

/// Scan the specified `filesystem` at `path` for [KernelPair] results.
fn scan_directory(filesystem: &mut FileSystem, path: &str) -> Result<Vec<KernelPair>> {
    // All the discovered kernel pairs.
    let mut pairs = Vec::new();

    // Construct a filesystem path from the path string.
    let path = CString16::try_from(path).context("unable to convert path to CString16")?;
    let path = Path::new(&path);
    let path = path.to_path_buf();

    // Check if the path exists and is a directory.
    let exists = filesystem
        .metadata(&path)
        .ok()
        .map(|metadata| metadata.is_directory())
        .unwrap_or(false);

    // If the path does not exist, return an empty list.
    if !exists {
        return Ok(pairs);
    }

    // Open a directory iterator on the path to scan.
    // Ignore errors here as in some scenarios this might fail due to symlinks.
    let Some(directory) = filesystem.read_dir(&path).ok() else {
        return Ok(pairs);
    };

    // For each item in the directory, find a kernel.
    for item in directory {
        let item = item.context("unable to read directory item")?;

        // Skip over any items that are not regular files.
        if !item.is_regular_file() {
            continue;
        }

        // Convert the name from a CString16 to a String.
        let name = item.file_name().to_string();

        // Find a kernel prefix that matches, if any.
        let Some(prefix) = KERNEL_PREFIXES
            .iter()
            .find(|prefix| name == **prefix || name.starts_with(&format!("{}-", prefix)))
        else {
            // Skip over anything that doesn't match a kernel prefix.
            continue;
        };

        // Acquire the suffix of the name, this will be used to match an initramfs.
        let suffix = &name[prefix.len()..];

        // Find a matching initramfs, if any.
        let mut initramfs_prefix_iter = INITRAMFS_PREFIXES.iter();
        let matched_initramfs_path = loop {
            let Some(prefix) = initramfs_prefix_iter.next() else {
                break None;
            };
            // Construct an initramfs path.
            let initramfs = format!("{}{}", prefix, suffix);
            let initramfs = CString16::try_from(initramfs.as_str())
                .context("unable to convert initramfs name to CString16")?;
            let mut initramfs_path = path.clone();
            initramfs_path.push(Path::new(&initramfs));
            // Check if the initramfs path exists, if it does, break out of the loop.
            if filesystem
                .try_exists(&initramfs_path)
                .context("unable to check if initramfs path exists")?
            {
                break Some(initramfs_path);
            }
        };

        // Construct a kernel path from the kernel name.
        let mut kernel = path.clone();
        kernel.push(Path::new(&item.file_name()));
        let kernel = kernel.to_string();
        let initramfs = matched_initramfs_path.map(|initramfs_path| initramfs_path.to_string());

        // Produce a kernel pair.
        let pair = KernelPair { kernel, initramfs };
        pairs.push(pair);
    }

    Ok(pairs)
}

/// Scan the specified `filesystem` for Linux kernels and matching initramfs.
pub fn scan(
    filesystem: &mut FileSystem,
    root: &DevicePath,
    config: &mut RootConfiguration,
) -> Result<bool> {
    let mut pairs = Vec::new();

    // Convert the device path root to a string we can use in the configuration.
    let mut root = root
        .to_string(DisplayOnly(false), AllowShortcuts(false))
        .context("unable to convert device root to string")?
        .to_string();
    // Add a trailing forward-slash to the root to ensure the device root is completed.
    root.push('/');

    // Generate a unique hash of the root path.
    let root_unique_hash = utils::unique_hash(&root);

    // Scan all locations for kernel pairs, adding them to the list.
    for location in SCAN_LOCATIONS {
        let scanned = scan_directory(filesystem, location)
            .with_context(|| format!("unable to scan directory {}", location))?;
        pairs.extend(scanned);
    }

    // If no kernel pairs were found, return false.
    if pairs.is_empty() {
        return Ok(false);
    }

    // Generate a unique name for the linux chainload action.
    let chainload_action_name = format!("{}{}", LINUX_CHAINLOAD_ACTION_PREFIX, root_unique_hash,);

    // Kernel pairs are detected, generate a list configuration for it.
    let generator = ListConfiguration {
        entry: EntryDeclaration {
            title: "Boot Linux $name".to_string(),
            actions: vec![chainload_action_name.clone()],
            ..Default::default()
        },
        values: pairs
            .into_iter()
            .map(|pair| {
                BTreeMap::from_iter(vec![
                    ("name".to_string(), pair.kernel.clone()),
                    ("kernel".to_string(), format!("{}{}", root, pair.kernel)),
                    (
                        "initrd".to_string(),
                        pair.initramfs
                            .map(|initramfs| format!("{}{}", root, initramfs))
                            .unwrap_or_default(),
                    ),
                ])
            })
            .collect(),
    };

    // Generate a unique name for the Linux generator and insert the generator into the configuration.
    config.generators.insert(
        format!("autoconfigure-linux-{}", root_unique_hash),
        GeneratorDeclaration {
            list: Some(generator),
            ..Default::default()
        },
    );

    // Insert a default value for the linux-options if it doesn't exist.
    if !config.values.contains_key("linux-options") {
        config
            .values
            .insert("linux-options".to_string(), "".to_string());
    }

    // Generate a chainload configuration for the list generator.
    // The list will provide these values to us.
    // Note that we don't need an extra \\ in the paths here.
    // The root already contains a trailing slash.
    let chainload = ChainloadConfiguration {
        path: "$kernel".to_string(),
        options: vec!["$linux-options".to_string()],
        linux_initrd: Some("$initrd".to_string()),
    };

    // Insert the chainload action into the configuration.
    config.actions.insert(
        chainload_action_name,
        ActionDeclaration {
            chainload: Some(chainload),
            ..Default::default()
        },
    );

    info!("{:?}", config);

    // We had a Linux kernel, so return true to indicate something was found.
    Ok(true)
}

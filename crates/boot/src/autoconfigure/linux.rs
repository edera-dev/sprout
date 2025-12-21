use crate::utils;
use crate::utils::vercmp;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::{format, vec};
use anyhow::{Context, Result};
use edera_sprout_config::RootConfiguration;
use edera_sprout_config::actions::ActionDeclaration;
use edera_sprout_config::actions::chainload::ChainloadConfiguration;
use edera_sprout_config::entries::EntryDeclaration;
use edera_sprout_config::generators::GeneratorDeclaration;
use edera_sprout_config::generators::list::ListConfiguration;
use uefi::CString16;
use uefi::fs::{FileSystem, Path, PathBuf};
use uefi::proto::device_path::DevicePath;
use uefi::proto::device_path::text::{AllowShortcuts, DisplayOnly};

/// The name prefix of the Linux chainload action that will be used to boot Linux.
const LINUX_CHAINLOAD_ACTION_PREFIX: &str = "linux-chainload-";

/// The locations to scan for kernel pairs.
/// We will check for symlinks and if this directory is a symlink, we will skip it.
/// The empty string represents the root of the filesystem.
const SCAN_LOCATIONS: &[&str] = &["\\boot", "\\"];

/// Prefixes of kernel files to scan for.
const KERNEL_PREFIXES: &[&str] = &["vmlinuz", "Image"];

/// Prefixes of initramfs files to match to.
const INITRAMFS_PREFIXES: &[&str] = &["initramfs", "initrd", "initrd.img"];

/// This is really silly, but if what we are booting is the Canonical stubble stub,
/// there is a chance it will assert that the load options are non-empty.
/// Technically speaking, load options can be empty. However, it assumes load options
/// have something in it. Canonical's stubble copied code from systemd that does this
/// and then uses that code improperly by asserting that the pointer is non-null.
/// To give a good user experience, we place a placeholder value here to ensure it's non-empty.
/// For stubble, this code ensures the command line pointer becomes null:
/// <https://github.com/ubuntu/stubble/blob/e56643979addfb98982266018e08921c07424a0c/stub.c#L61-L64>
/// Then this code asserts on it, stopping the boot process:
/// <https://github.com/ubuntu/stubble/blob/e56643979addfb98982266018e08921c07424a0c/stub.c#L27>
const DEFAULT_LINUX_OPTIONS: &str = "placeholder";

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

    // We have to special-case the root directory due to path logic in the uefi crate.
    let is_root = path.is_empty() || path == "\\";

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

    // Create a new path used for joining file names below.
    // All attempts to derive paths for the files in the directory should use this instead.
    // The uefi crate does not handle push correctly for the root directory.
    // It will add a second slash, which will cause our path logic to fail.
    let path_for_join = if is_root {
        PathBuf::new()
    } else {
        path.clone()
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

        // Convert the name to lowercase to make all of this case-insensitive.
        let name_for_match = name.to_lowercase();

        // Find a kernel prefix that matches, if any.
        // This is case-insensitive to ensure we pick up all possibilities.
        let Some(prefix) = KERNEL_PREFIXES.iter().find(|prefix| {
            name_for_match == **prefix || name_for_match.starts_with(&format!("{}-", prefix))
        }) else {
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
            let mut initramfs_path = path_for_join.clone();
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
        let mut kernel = path_for_join.clone();
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

    // Sort the kernel pairs by kernel version, if it has one, newer kernels first.
    pairs.sort_by(|a, b| vercmp::compare_versions(&a.kernel, &b.kernel).reverse());

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
        format!("auto-linux-{}", root_unique_hash),
        GeneratorDeclaration {
            list: Some(generator),
            ..Default::default()
        },
    );

    // Insert a default value for the linux-options if it doesn't exist.
    if !config.values.contains_key("linux-options") {
        config.values.insert(
            "linux-options".to_string(),
            DEFAULT_LINUX_OPTIONS.to_string(),
        );
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

    // We had a Linux kernel, so return true to indicate something was found.
    Ok(true)
}

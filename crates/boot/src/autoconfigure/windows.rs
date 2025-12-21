use crate::utils;
use alloc::string::ToString;
use alloc::{format, vec};
use anyhow::{Context, Result};
use edera_sprout_config::RootConfiguration;
use edera_sprout_config::actions::ActionDeclaration;
use edera_sprout_config::actions::chainload::ChainloadConfiguration;
use edera_sprout_config::entries::EntryDeclaration;
use uefi::CString16;
use uefi::fs::{FileSystem, Path};
use uefi::proto::device_path::DevicePath;
use uefi::proto::device_path::text::{AllowShortcuts, DisplayOnly};

/// The name prefix of the Windows chainload action that will be used to boot Windows.
const WINDOWS_CHAINLOAD_ACTION_PREFIX: &str = "windows-chainload-";

/// Windows boot manager path.
const BOOTMGR_FW_PATH: &str = "\\EFI\\Microsoft\\Boot\\bootmgfw.efi";

/// Scan the specified `filesystem` for Windows configurations.
pub fn scan(
    filesystem: &mut FileSystem,
    root: &DevicePath,
    config: &mut RootConfiguration,
) -> Result<bool> {
    // Convert the boot manager firmware path to a path.
    let bootmgr_fw_path =
        CString16::try_from(BOOTMGR_FW_PATH).context("unable to convert path to CString16")?;
    let bootmgr_fw_path = Path::new(&bootmgr_fw_path);

    // Check if the boot manager firmware path exists, if it doesn't, return false.
    if !filesystem
        .try_exists(bootmgr_fw_path)
        .context("unable to check if bootmgr firmware path exists")?
    {
        return Ok(false);
    }

    // Convert the device path root to a string we can use in the configuration.
    let mut root = root
        .to_string(DisplayOnly(false), AllowShortcuts(false))
        .context("unable to convert device root to string")?
        .to_string();
    // Add a trailing forward-slash to the root to ensure the device root is completed.
    root.push('/');

    // Generate a unique hash of the root path.
    let root_unique_hash = utils::unique_hash(&root);

    // Generate a unique name for the Windows chainload action.
    let chainload_action_name = format!("{}{}", WINDOWS_CHAINLOAD_ACTION_PREFIX, root_unique_hash,);

    // Generate an entry name for Windows.
    let entry_name = format!("auto-windows-{}", root_unique_hash);

    // Create an entry for Windows and insert it into the configuration.
    let entry = EntryDeclaration {
        title: "Boot Windows".to_string(),
        actions: vec![chainload_action_name.clone()],
        values: Default::default(),
        sort_key: None, // Use the default sort key.
    };
    config.entries.insert(entry_name, entry);

    // Generate a chainload configuration for Windows.
    let chainload = ChainloadConfiguration {
        path: format!("{}{}", root, bootmgr_fw_path),
        options: vec![],
        ..Default::default()
    };

    // Insert the chainload action into the configuration.
    config.actions.insert(
        chainload_action_name,
        ActionDeclaration {
            chainload: Some(chainload),
            ..Default::default()
        },
    );

    // We have a Windows boot entry, so return true to indicate something was found.
    Ok(true)
}

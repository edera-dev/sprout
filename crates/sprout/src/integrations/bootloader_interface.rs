use crate::integrations::bootloader_interface::bitflags::LoaderFeatures;
use crate::platform::timer::PlatformTimer;
use crate::utils::device_path_subpath;
use crate::utils::variables::{VariableClass, VariableController};
use anyhow::{Context, Result};
use uefi::proto::device_path::DevicePath;
use uefi::{Guid, guid};
use uefi_raw::table::runtime::VariableVendor;

/// bitflags: LoaderFeatures bitflags.
mod bitflags;

/// The name of the bootloader to tell the system.
const LOADER_NAME: &str = "Sprout";

/// Represents the configured timeout for the bootloader interface.
pub enum BootloaderInterfaceTimeout {
    /// Force the menu to be shown.
    MenuForce,
    /// Hide the menu.
    MenuHidden,
    /// Disable the menu.
    MenuDisabled,
    /// Set a timeout for the menu.
    Timeout(u64),
    /// Timeout is unspecified.
    Unspecified,
}

/// Bootloader Interface support.
pub struct BootloaderInterface;

impl BootloaderInterface {
    /// Bootloader Interface GUID from https://systemd.io/BOOT_LOADER_INTERFACE
    const VENDOR: VariableController = VariableController::new(VariableVendor(guid!(
        "4a67b082-0a4c-41cf-b6c7-440b29bb8c4f"
    )));

    /// The feature we support in Sprout.
    fn features() -> LoaderFeatures {
        LoaderFeatures::Xbootldr
            | LoaderFeatures::LoadDriver
            | LoaderFeatures::Tpm2ActivePcrBanks
            | LoaderFeatures::RetainShim
            | LoaderFeatures::ConfigTimeout
            | LoaderFeatures::ConfigTimeoutOneShot
            | LoaderFeatures::MenuDisable
            | LoaderFeatures::EntryDefault
            | LoaderFeatures::EntryOneShot
    }

    /// Tell the system that Sprout was initialized at the current time.
    pub fn mark_init(timer: &PlatformTimer) -> Result<()> {
        Self::mark_time("LoaderTimeInitUSec", timer)
    }

    /// Tell the system that Sprout is about to execute the boot entry.
    pub fn mark_exec(timer: &PlatformTimer) -> Result<()> {
        Self::mark_time("LoaderTimeExecUSec", timer)
    }

    /// Tell the system that Sprout is about to display the menu.
    pub fn mark_menu(timer: &PlatformTimer) -> Result<()> {
        Self::mark_time("LoaderTimeMenuUSec", timer)
    }

    /// Tell the system about the current time as measured by the platform timer.
    /// Sets the variable specified by `key` to the number of microseconds.
    fn mark_time(key: &str, timer: &PlatformTimer) -> Result<()> {
        // Measure the elapsed time since the hardware timer was started.
        let elapsed = timer.elapsed_since_lifetime();
        Self::VENDOR.set_cstr16(
            key,
            &elapsed.as_micros().to_string(),
            VariableClass::BootAndRuntimeTemporary,
        )
    }

    /// Tell the system what loader is being used and our features.
    pub fn set_loader_info() -> Result<()> {
        // Set the LoaderInfo variable with the name of the loader.
        Self::VENDOR
            .set_cstr16(
                "LoaderInfo",
                LOADER_NAME,
                VariableClass::BootAndRuntimeTemporary,
            )
            .context("unable to set loader info variable")?;

        // Set the LoaderFeatures variable with the features we support.
        Self::VENDOR
            .set_u64le(
                "LoaderFeatures",
                Self::features().bits(),
                VariableClass::BootAndRuntimeTemporary,
            )
            .context("unable to set loader features variable")?;
        Ok(())
    }

    /// Tell the system the relative path to the partition root of the current bootloader.
    pub fn set_loader_path(path: &DevicePath) -> Result<()> {
        let subpath = device_path_subpath(path).context("unable to get loader path subpath")?;
        Self::VENDOR.set_cstr16(
            "LoaderImageIdentifier",
            &subpath,
            VariableClass::BootAndRuntimeTemporary,
        )
    }

    /// Tell the system what the partition GUID of the ESP Sprout was booted from is.
    pub fn set_partition_guid(guid: &Guid) -> Result<()> {
        Self::VENDOR.set_cstr16(
            "LoaderDevicePartUUID",
            &guid.to_string(),
            VariableClass::BootAndRuntimeTemporary,
        )
    }

    /// Tell the system what boot entries are available.
    pub fn set_entries<N: AsRef<str>>(entries: impl Iterator<Item = N>) -> Result<()> {
        // Entries are stored as a null-terminated list of CString16 strings back to back.
        // Iterate over the entries and convert them to CString16 placing them into data.
        let mut data = Vec::new();
        for entry in entries {
            // Convert the entry to CString16 little endian.
            let encoded = entry
                .as_ref()
                .encode_utf16()
                .flat_map(|c| c.to_le_bytes())
                .collect::<Vec<u8>>();
            // Write the bytes into the data buffer.
            data.extend_from_slice(&encoded);
            // Add a null terminator to the end of the entry.
            data.extend_from_slice(&[0, 0]);
        }

        // If no data was generated, we will do nothing.
        if data.is_empty() {
            return Ok(());
        }

        Self::VENDOR.set(
            "LoaderEntries",
            &data,
            VariableClass::BootAndRuntimeTemporary,
        )
    }

    /// Tell the system what the selected boot entry is.
    pub fn set_selected_entry(entry: String) -> Result<()> {
        Self::VENDOR.set_cstr16(
            "LoaderEntrySelected",
            &entry,
            VariableClass::BootAndRuntimeTemporary,
        )
    }

    /// Tell the system about the UEFI firmware we are running on.
    pub fn set_firmware_info() -> Result<()> {
        // Access the firmware revision.
        let firmware_revision = uefi::system::firmware_revision();

        // Access the UEFI revision.
        let uefi_revision = uefi::system::uefi_revision();

        // Format the firmware information string into something human-readable.
        let firmware_info = format!(
            "{} {}.{:02}",
            uefi::system::firmware_vendor(),
            firmware_revision >> 16,
            firmware_revision & 0xffff,
        );
        Self::VENDOR.set_cstr16(
            "LoaderFirmwareInfo",
            &firmware_info,
            VariableClass::BootAndRuntimeTemporary,
        )?;

        // Format the firmware revision into something human-readable.
        let firmware_type = format!(
            "UEFI {}.{:02}",
            uefi_revision.major(),
            uefi_revision.minor()
        );
        Self::VENDOR.set_cstr16(
            "LoaderFirmwareType",
            &firmware_type,
            VariableClass::BootAndRuntimeTemporary,
        )
    }

    /// Tell the system what the number of active PCR banks is.
    /// If this is zero, that is okay.
    pub fn set_tpm2_active_pcr_banks(value: u32) -> Result<()> {
        // Format the value into the specification format.
        let value = format!("0x{:08x}", value);
        Self::VENDOR.set_cstr16(
            "LoaderTpm2ActivePcrBanks",
            &value,
            VariableClass::BootAndRuntimeTemporary,
        )
    }

    /// Retrieve the timeout value from the bootloader interface, using the specified `key`.
    /// `remove` indicates whether, when found, we remove the variable.
    fn get_timeout_value(key: &str, remove: bool) -> Result<Option<BootloaderInterfaceTimeout>> {
        // Retrieve the timeout value from the bootloader interface.
        let Some(value) = Self::VENDOR
            .get_cstr16(key)
            .context("unable to get timeout value")?
        else {
            return Ok(None);
        };

        // If we reach here, we know the value was specified.
        // If `remove` is true, remove the variable.
        if remove {
            Self::VENDOR
                .remove(key)
                .context("unable to remove timeout variable")?;
        }

        // If the value is empty, return Unspecified.
        if value.is_empty() {
            return Ok(Some(BootloaderInterfaceTimeout::Unspecified));
        }

        // If the value is "menu-force", return MenuForce.
        if value == "menu-force" {
            return Ok(Some(BootloaderInterfaceTimeout::MenuForce));
        }

        // If the value is "menu-hidden", return MenuHidden.
        if value == "menu-hidden" {
            return Ok(Some(BootloaderInterfaceTimeout::MenuHidden));
        }

        // If the value is "menu-disabled", return MenuDisabled.
        if value == "menu-disabled" {
            return Ok(Some(BootloaderInterfaceTimeout::MenuDisabled));
        }

        // Parse the value as a u64 to decode an numeric value.
        let value = value
            .parse::<u64>()
            .context("unable to parse timeout value")?;

        // The specification says that a value of 0 means that the menu should be hidden.
        if value == 0 {
            return Ok(Some(BootloaderInterfaceTimeout::MenuHidden));
        }

        // If we reach here, we know it must be a real timeout value.
        Ok(Some(BootloaderInterfaceTimeout::Timeout(value)))
    }

    /// Get the timeout from the bootloader interface.
    /// This indicates how the menu should behave.
    /// If no values are set, Unspecified is returned.
    pub fn get_timeout() -> Result<BootloaderInterfaceTimeout> {
        // Attempt to acquire the value of the LoaderConfigTimeoutOneShot variable.
        // This should take precedence over the LoaderConfigTimeout variable.
        let oneshot = Self::get_timeout_value("LoaderConfigTimeoutOneShot", true)
            .context("unable to check for LoaderConfigTimeoutOneShot variable")?;

        // If oneshot was found, return it.
        if let Some(oneshot) = oneshot {
            return Ok(oneshot);
        }

        // Attempt to acquire the value of the LoaderConfigTimeout variable.
        // This will be used if the LoaderConfigTimeoutOneShot variable is not set.
        let direct = Self::get_timeout_value("LoaderConfigTimeout", false)
            .context("unable to check for LoaderConfigTimeout variable")?;

        // If direct was found, return it.
        if let Some(direct) = direct {
            return Ok(direct);
        }

        // If we reach here, we know that neither variable was set.
        // We provide the unspecified value instead.
        Ok(BootloaderInterfaceTimeout::Unspecified)
    }

    /// Get the default entry set by the bootloader interface.
    pub fn get_default_entry() -> Result<Option<String>> {
        Self::VENDOR
            .get_cstr16("LoaderEntryDefault")
            .context("unable to get default entry from bootloader interface")
    }

    /// Get the oneshot entry set by the bootloader interface.
    /// This should be the entry we boot.
    pub fn get_oneshot_entry() -> Result<Option<String>> {
        // Acquire the value of the LoaderEntryOneShot variable.
        // If it is not set, return None.
        let Some(value) = Self::VENDOR
            .get_cstr16("LoaderEntryOneShot")
            .context("unable to get oneshot entry from bootloader interface")?
        else {
            return Ok(None);
        };

        // Remove the oneshot entry from the bootloader interface.
        Self::VENDOR
            .remove("LoaderEntryOneShot")
            .context("unable to remove oneshot entry")?;

        // Return the oneshot value.
        Ok(Some(value))
    }
}

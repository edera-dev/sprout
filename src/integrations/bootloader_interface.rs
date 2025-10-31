use crate::platform::timer::PlatformTimer;
use crate::utils::device_path_subpath;
use crate::utils::variables::{VariableClass, VariableController};
use anyhow::{Context, Result};
use uefi::proto::device_path::DevicePath;
use uefi::{Guid, guid};
use uefi_raw::table::runtime::VariableVendor;

/// The name of the bootloader to tell the system.
const LOADER_NAME: &str = "Sprout";

/// Bootloader Interface support.
pub struct BootloaderInterface;

impl BootloaderInterface {
    /// Bootloader Interface GUID from https://systemd.io/BOOT_LOADER_INTERFACE
    const VENDOR: VariableController = VariableController::new(VariableVendor(guid!(
        "4a67b082-0a4c-41cf-b6c7-440b29bb8c4f"
    )));

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
        Self::mark_time("LoaderTimeMenuUsec", timer)
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

    /// Tell the system what loader is being used.
    pub fn set_loader_info() -> Result<()> {
        Self::VENDOR.set_cstr16(
            "LoaderInfo",
            LOADER_NAME,
            VariableClass::BootAndRuntimeTemporary,
        )
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
        Self::VENDOR.set(
            "LoaderEntries",
            &data,
            VariableClass::BootAndRuntimeTemporary,
        )
    }

    /// Tell the system what the default boot entry is.
    pub fn set_default_entry(entry: String) -> Result<()> {
        Self::VENDOR.set_cstr16(
            "LoaderEntryDefault",
            &entry,
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
        let revision = uefi::system::firmware_revision();

        // Format the firmware information string into something human-readable.
        let firmware_info = format!(
            "{} {}.{:02}",
            uefi::system::firmware_vendor(),
            revision >> 16,
            revision & 0xffff,
        );
        Self::VENDOR.set_cstr16(
            "LoaderFirmwareInfo",
            &firmware_info,
            VariableClass::BootAndRuntimeTemporary,
        )?;

        // Format the firmware revision into something human-readable.
        let firmware_type = format!("UEFI {}.{:02}", revision >> 16, revision & 0xffff);
        Self::VENDOR.set_cstr16(
            "LoaderFirmwareType",
            &firmware_type,
            VariableClass::BootAndRuntimeTemporary,
        )
    }
}

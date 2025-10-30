use crate::platform::timer::PlatformTimer;
use crate::utils::device_path_subpath;
use anyhow::{Context, Result};
use uefi::proto::device_path::DevicePath;
use uefi::{CString16, Guid, guid};
use uefi_raw::table::runtime::{VariableAttributes, VariableVendor};

/// Bootloader Interface support.
pub struct BootloaderInterface;

impl BootloaderInterface {
    /// Bootloader Interface GUID from https://systemd.io/BOOT_LOADER_INTERFACE
    const VENDOR: VariableVendor = VariableVendor(guid!("4a67b082-0a4c-41cf-b6c7-440b29bb8c4f"));

    /// Tell the system that Sprout was initialized at the current time.
    pub fn mark_init(timer: &PlatformTimer) -> Result<()> {
        Self::mark_time("LoaderTimeInitUSec", timer)
    }

    /// Tell the system that Sprout is about to execute the boot entry.
    pub fn mark_exec(timer: &PlatformTimer) -> Result<()> {
        Self::mark_time("LoaderTimeExecUSec", timer)
    }

    /// Tell the system about the current time as measured by the platform timer.
    /// Sets the variable specified by `key` to the number of microseconds.
    fn mark_time(key: &str, timer: &PlatformTimer) -> Result<()> {
        // Measure the elapsed time since the hardware timer was started.
        let elapsed = timer.elapsed_since_lifetime();
        Self::set_cstr16(key, &elapsed.as_micros().to_string())
    }

    /// Tell the system the relative path to the partition root of the current bootloader.
    pub fn set_loader_path(path: &DevicePath) -> Result<()> {
        let subpath = device_path_subpath(path).context("unable to get loader path subpath")?;
        Self::set_cstr16("LoaderImageIdentifier", &subpath)
    }

    /// Tell the system what the partition GUID of the ESP Sprout was booted from is.
    pub fn set_partition_guid(guid: &Guid) -> Result<()> {
        Self::set_cstr16("LoaderDevicePartUUID", &guid.to_string())
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
            // Write the bytes (including the null terminator) into the data buffer.
            data.extend_from_slice(&encoded);
        }
        Self::set("LoaderEntries", &data)
    }

    /// Tell the system what the default boot entry is.
    pub fn set_default_entry(entry: String) -> Result<()> {
        Self::set_cstr16("LoaderEntryDefault", &entry)
    }

    /// Tell the system what the selected boot entry is.
    pub fn set_selected_entry(entry: String) -> Result<()> {
        Self::set_cstr16("LoaderEntrySelected", &entry)
    }

    /// Tell the system about the UEFI firmware we are running on.
    pub fn set_firmware_info() -> Result<()> {
        // Format the firmware information string into something human-readable.
        let firmware_info = format!(
            "{} {}.{:02}",
            uefi::system::firmware_vendor(),
            uefi::system::firmware_revision() >> 16,
            uefi::system::firmware_revision() & 0xFFFFF,
        );
        Self::set_cstr16("LoaderFirmwareInfo", &firmware_info)?;

        // Format the firmware revision into something human-readable.
        let firmware_type = format!("UEFI {:02}", uefi::system::firmware_revision());
        Self::set_cstr16("LoaderFirmwareType", &firmware_type)
    }

    /// The [VariableAttributes] for bootloader interface variables.
    fn attributes() -> VariableAttributes {
        VariableAttributes::BOOTSERVICE_ACCESS | VariableAttributes::RUNTIME_ACCESS
    }

    /// Set a bootloader interface variable specified by `key` to `value`.
    fn set(key: &str, value: &[u8]) -> Result<()> {
        let name =
            CString16::try_from(key).context("unable to convert variable name to CString16")?;
        uefi::runtime::set_variable(&name, &Self::VENDOR, Self::attributes(), value)
            .with_context(|| format!("unable to set efi variable {}", key))?;
        Ok(())
    }

    /// Set a bootloader interface variable specified by `key` to `value`, converting the value to
    /// a [CString16].
    fn set_cstr16(key: &str, value: &str) -> Result<()> {
        // Encode the value as a CString16 little endian.
        let encoded = value
            .encode_utf16()
            .flat_map(|c| c.to_le_bytes())
            .collect::<Vec<u8>>();
        Self::set(key, &encoded)
    }
}

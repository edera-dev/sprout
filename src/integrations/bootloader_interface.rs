use anyhow::Result;
use uefi::Guid;

/// Bootloader Interface support.
pub struct BootloaderInterface;

impl BootloaderInterface {
    /// Tell the system that Sprout was initialized at the current time.
    pub fn mark_init() -> Result<()> {
        // TODO(azenla): Implement support for LoaderTimeInitUSec here.
        Ok(())
    }

    /// Tell the system that Sprout is about to execute the boot entry.
    pub fn mark_exec() -> Result<()> {
        // TODO(azenla): Implement support for LoaderTimeExecUSec here.
        Ok(())
    }

    /// Tell the system what the partition GUID of the ESP Sprout was booted from is.
    pub fn set_partition_guid(_guid: &Guid) -> Result<()> {
        // TODO(azenla): Implement support for LoaderDevicePartUUID here.
        Ok(())
    }

    /// Tell the system what boot entries are available.
    pub fn set_entries<N: AsRef<str>>(_entries: impl Iterator<Item = N>) -> Result<()> {
        // TODO(azenla): Implement support for LoaderEntries here.
        Ok(())
    }

    /// Tell the system what the default boot entry is.
    pub fn set_default_entry(_entry: String) -> Result<()> {
        // TODO(azenla): Implement support for LoaderEntryDefault here.
        Ok(())
    }

    /// Tell the system what the selected boot entry is.
    pub fn set_selected_entry(_entry: String) -> Result<()> {
        // TODO(azenla): Implement support for LoaderEntrySelected here.
        Ok(())
    }
}

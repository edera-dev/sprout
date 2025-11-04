use anyhow::{Context, Result};
use uefi::Guid;
use uefi::proto::device_path::DevicePath;
use uefi::proto::media::partition::PartitionInfo;
use uefi_raw::Status;

/// Represents the type of partition GUID that can be retrieved.
#[derive(PartialEq, Eq)]
pub enum PartitionGuidForm {
    /// The partition GUID is the unique partition GUID.
    Partition,
    /// The partition GUID is the partition type GUID.
    PartitionType,
}

/// Retrieve the partition / partition type GUID of the device root `path`.
/// This only works on GPT partitions. If the root is not a GPT partition, None is returned.
/// If the GUID is all zeros, this will return None.
pub fn partition_guid(path: &DevicePath, form: PartitionGuidForm) -> Result<Option<Guid>> {
    // Clone the path so we can pass it to the UEFI stack.
    let path = path.to_boxed();
    let result = uefi::boot::locate_device_path::<PartitionInfo>(&mut &*path);
    let handle = match result {
        Ok(handle) => Ok(Some(handle)),
        Err(error) => {
            // If the error is NOT_FOUND or UNSUPPORTED, we can return None.
            // These are non-fatal errors.
            if error.status() == Status::NOT_FOUND || error.status() == Status::UNSUPPORTED {
                Ok(None)
            } else {
                Err(error)
            }
        }
    }
    .context("unable to locate device path")?;

    // If we have the handle, we can try to open the partition info protocol.
    if let Some(handle) = handle {
        // Open the partition info protocol.
        let partition_info = uefi::boot::open_protocol_exclusive::<PartitionInfo>(handle)
            .context("unable to open partition info protocol")?;
        // Find the unique partition GUID.
        // If this is not a GPT partition, this will produce None.
        Ok(partition_info
            .gpt_partition_entry()
            .map(|entry| match form {
                // Match the form of the partition GUID.
                PartitionGuidForm::Partition => entry.unique_partition_guid,
                PartitionGuidForm::PartitionType => entry.partition_type_guid.0,
            })
            .filter(|guid| !guid.is_zero()))
    } else {
        Ok(None)
    }
}

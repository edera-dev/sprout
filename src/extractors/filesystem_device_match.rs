use crate::context::SproutContext;
use crate::utils;
use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use std::rc::Rc;
use std::str::FromStr;
use uefi::fs::{FileSystem, Path};
use uefi::proto::device_path::DevicePath;
use uefi::proto::media::file::{File, FileSystemVolumeLabel};
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::proto::media::partition::PartitionInfo;
use uefi::{CString16, Guid};
use uefi_raw::Status;

/// The filesystem device match extractor.
/// This extractor finds a filesystem using some search criteria and returns
/// the device root path that can concatenated with subpaths to access files
/// on a particular filesystem.
///
/// This function only requires one of the criteria to match.
/// The fallback value can be used to provide a value if none is found.
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct FilesystemDeviceMatchExtractor {
    /// Matches a filesystem that has the specified label.
    #[serde(default, rename = "has-label")]
    pub has_label: Option<String>,
    /// Matches a filesystem that has the specified item.
    /// An item is either a directory or file.
    #[serde(default, rename = "has-item")]
    pub has_item: Option<String>,
    /// Matches a filesystem that has the specified partition UUID.
    #[serde(default, rename = "has-partition-uuid")]
    pub has_partition_uuid: Option<String>,
    /// Matches a filesystem that has the specified partition type UUID.
    #[serde(default, rename = "has-partition-type-uuid")]
    pub has_partition_type_uuid: Option<String>,
    /// The fallback value to use if no filesystem matches the criteria.
    #[serde(default)]
    pub fallback: Option<String>,
}

/// Extract a filesystem device path using the specified `context` and `extractor` configuration.
pub fn extract(
    context: Rc<SproutContext>,
    extractor: &FilesystemDeviceMatchExtractor,
) -> Result<String> {
    // Find all the filesystems inside the UEFI stack.
    let handles = uefi::boot::find_handles::<SimpleFileSystem>()
        .context("unable to find filesystem handles")?;

    // Iterate over all the filesystems and check if they match the criteria.
    for handle in handles {
        // This defines whether a match has been found.
        let mut has_match = false;

        // Extract the partition info for this filesystem.
        // There is no guarantee that the filesystem has a partition.
        let partition_info = {
            // Open the partition info protocol for this handle.
            let partition_info = uefi::boot::open_protocol_exclusive::<PartitionInfo>(handle);

            match partition_info {
                Ok(partition_info) => {
                    // GPT partitions have a unique partition GUID.
                    // MBR does not.
                    if let Some(gpt) = partition_info.gpt_partition_entry() {
                        let uuid = gpt.unique_partition_guid;
                        let type_uuid = gpt.partition_type_guid;
                        Some((uuid, type_uuid.0))
                    } else {
                        None
                    }
                }

                Err(error) => {
                    // If the filesystem does not have a partition, that is okay.
                    if error.status() == Status::NOT_FOUND || error.status() == Status::UNSUPPORTED
                    {
                        None
                    } else {
                        // We should still handle other errors gracefully.
                        Err(error).context("unable to open filesystem partition info")?;
                        unreachable!()
                    }
                }
            }
        };

        // Check if the partition info matches partition uuid criteria.
        if let Some((partition_uuid, _partition_type_guid)) = partition_info
            && let Some(ref has_partition_uuid) = extractor.has_partition_uuid
        {
            let parsed_uuid = Guid::from_str(has_partition_uuid)
                .map_err(|e| anyhow!("unable to parse has-partition-uuid: {}", e))?;
            if partition_uuid != parsed_uuid {
                continue;
            }
            has_match = true;
        }

        // Check if the partition info matches partition type uuid criteria.
        if let Some((_partition_uuid, partition_type_guid)) = partition_info
            && let Some(ref has_partition_type_uuid) = extractor.has_partition_type_uuid
        {
            let parsed_uuid = Guid::from_str(has_partition_type_uuid)
                .map_err(|e| anyhow!("unable to parse has-partition-type-uuid: {}", e))?;
            if partition_type_guid != parsed_uuid {
                continue;
            }
            has_match = true;
        }

        // Open the filesystem protocol for this handle.
        let mut filesystem = uefi::boot::open_protocol_exclusive::<SimpleFileSystem>(handle)
            .context("unable to open filesystem protocol")?;

        // Check if the filesystem matches label criteria.
        if let Some(ref label) = extractor.has_label {
            let want_label = CString16::try_from(context.stamp(label).as_str())
                .context("unable to convert label to CString16")?;
            let mut root = filesystem
                .open_volume()
                .context("unable to open filesystem volume")?;
            let label = root
                .get_boxed_info::<FileSystemVolumeLabel>()
                .context("unable to get filesystem volume label")?;

            if label.volume_label() != want_label {
                continue;
            }
            has_match = true;
        }

        // Check if the filesystem matches item criteria.
        if let Some(ref item) = extractor.has_item {
            let want_item = CString16::try_from(context.stamp(item).as_str())
                .context("unable to convert item to CString16")?;
            let mut filesystem = FileSystem::new(filesystem);

            // Check the metadata of the item.
            let metadata = filesystem.metadata(Path::new(&want_item));

            // Ignore filesystem errors as we can't do anything useful with the error.
            if metadata.is_err() {
                continue;
            }

            let metadata = metadata?;
            // Only check directories and files.
            if !(metadata.is_directory() || metadata.is_regular_file()) {
                continue;
            }
            has_match = true;
        }

        // If there is no match, continue to the next filesystem.
        if !has_match {
            continue;
        }

        // If we have a match, return the device root path.
        let path = uefi::boot::open_protocol_exclusive::<DevicePath>(handle)
            .context("unable to open filesystem device path")?;
        let path = path.deref();
        // Acquire the device path root as a string.
        return utils::device_path_root(path).context("unable to get device path root");
    }

    // If there is a fallback value, use it at this point.
    if let Some(fallback) = &extractor.fallback {
        return Ok(fallback.clone());
    }

    // Without a fallback, we can't continue, so bail.
    bail!("unable to find matching filesystem")
}

use crate::context::SproutContext;
use crate::utils;
use alloc::rc::Rc;
use alloc::string::String;
use anyhow::{Context, Result, anyhow, bail};
use core::ops::Deref;
use core::str::FromStr;
use edera_sprout_config::extractors::filesystem_device_match::FilesystemDeviceMatchExtractor;
use uefi::fs::{FileSystem, Path};
use uefi::proto::device_path::DevicePath;
use uefi::proto::media::file::{File, FileSystemVolumeLabel};
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::{CString16, Guid};

/// Extract a filesystem device path using the specified `context` and `extractor` configuration.
pub fn extract(
    context: Rc<SproutContext>,
    extractor: &FilesystemDeviceMatchExtractor,
) -> Result<String> {
    // If no criteria are provided, bail with an error.
    if extractor.has_label.is_none()
        && extractor.has_item.is_none()
        && extractor.has_partition_uuid.is_none()
        && extractor.has_partition_type_uuid.is_none()
    {
        bail!("at least one criteria is required for filesystem-device-match");
    }

    // Find all the filesystems inside the UEFI stack.
    let handles = uefi::boot::find_handles::<SimpleFileSystem>()
        .context("unable to find filesystem handles")?;

    // Iterate over all the filesystems and check if they match the criteria.
    for handle in handles {
        // This defines whether a match has been found.
        let mut has_match = false;

        // Check if the partition info matches partition uuid criteria.
        if let Some(ref has_partition_uuid) = extractor.has_partition_uuid {
            // Parse the partition uuid from the extractor.
            let parsed_uuid = Guid::from_str(has_partition_uuid)
                .map_err(|e| anyhow!("unable to parse has-partition-uuid: {}", e))?;

            // Fetch the root of the device.
            let root = uefi::boot::open_protocol_exclusive::<DevicePath>(handle)
                .context("unable to fetch the device path of the filesystem")?
                .deref()
                .to_boxed();

            // Fetch the partition uuid for this filesystem.
            let partition_uuid = utils::partition_guid(&root, utils::PartitionGuidForm::Partition)
                .context("unable to fetch the partition uuid of the filesystem")?;

            // Compare the partition uuid to the parsed uuid.
            // If it does not match, continue to the next filesystem.
            if partition_uuid != Some(parsed_uuid) {
                continue;
            }
            has_match = true;
        }

        // Check if the partition info matches partition type uuid criteria.
        if let Some(ref has_partition_type_uuid) = extractor.has_partition_type_uuid {
            // Parse the partition type uuid from the extractor.
            let parsed_uuid = Guid::from_str(has_partition_type_uuid)
                .map_err(|e| anyhow!("unable to parse has-partition-type-uuid: {}", e))?;

            // Fetch the root of the device.
            let root = uefi::boot::open_protocol_exclusive::<DevicePath>(handle)
                .context("unable to fetch the device path of the filesystem")?
                .deref()
                .to_boxed();

            // Fetch the partition type uuid for this filesystem.
            let partition_type_uuid =
                utils::partition_guid(&root, utils::PartitionGuidForm::PartitionType)
                    .context("unable to fetch the partition uuid of the filesystem")?;
            // Compare the partition type uuid to the parsed uuid.
            // If it does not match, continue to the next filesystem.
            if partition_type_uuid != Some(parsed_uuid) {
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
            // Ignore filesystem errors as we can't do anything useful with the error.
            let Some(metadata) = filesystem.metadata(Path::new(&want_item)).ok() else {
                continue;
            };

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

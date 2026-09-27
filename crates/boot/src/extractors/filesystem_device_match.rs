use crate::context::SproutContext;
use alloc::rc::Rc;
use alloc::string::String;
use anyhow::{Context, Result, anyhow, bail};
use core::ops::Deref;
use core::str::FromStr;
use edera_sprout_config::extractors::filesystem_device_match::FilesystemDeviceMatchExtractor;
use eficore::partition::PartitionGuidForm;
use log::warn;
use uefi::fs::{FileSystem, Path};
use uefi::proto::device_path::DevicePath;
use uefi::proto::media::file::{File, FileSystemVolumeLabel};
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::{CString16, Guid, Handle};

/// Check whether the filesystem at `handle` matches all the criteria of the `extractor`
/// configuration inside the `context`. The partition UUIDs are parsed ahead of time into
/// `partition_uuid` and `partition_type_uuid`.
fn is_match(
    context: &Rc<SproutContext>,
    extractor: &FilesystemDeviceMatchExtractor,
    handle: Handle,
    partition_uuid: Option<Guid>,
    partition_type_uuid: Option<Guid>,
) -> Result<bool> {
    // Check if the partition info matches partition uuid criteria.
    if let Some(parsed_uuid) = partition_uuid {
        // Fetch the root of the device.
        let root = uefi::boot::open_protocol_exclusive::<DevicePath>(handle)
            .context("unable to fetch the device path of the filesystem")?
            .deref()
            .to_boxed();

        // Fetch the partition uuid for this filesystem.
        let partition_uuid =
            eficore::partition::partition_guid(&root, PartitionGuidForm::Partition)
                .context("unable to fetch the partition uuid of the filesystem")?;

        // Compare the partition uuid to the parsed uuid.
        if partition_uuid != Some(parsed_uuid) {
            return Ok(false);
        }
    }

    // Check if the partition info matches partition type uuid criteria.
    if let Some(parsed_uuid) = partition_type_uuid {
        // Fetch the root of the device.
        let root = uefi::boot::open_protocol_exclusive::<DevicePath>(handle)
            .context("unable to fetch the device path of the filesystem")?
            .deref()
            .to_boxed();

        // Fetch the partition type uuid for this filesystem.
        let partition_type_uuid =
            eficore::partition::partition_guid(&root, PartitionGuidForm::PartitionType)
                .context("unable to fetch the partition uuid of the filesystem")?;

        // Compare the partition type uuid to the parsed uuid.
        if partition_type_uuid != Some(parsed_uuid) {
            return Ok(false);
        }
    }

    // The remaining criteria need the filesystem itself.
    if extractor.has_label.is_none() && extractor.has_item.is_none() {
        return Ok(true);
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
            return Ok(false);
        }
    }

    // Check if the filesystem matches item criteria.
    if let Some(ref item) = extractor.has_item {
        let want_item = CString16::try_from(context.stamp(item).as_str())
            .context("unable to convert item to CString16")?;
        let mut filesystem = FileSystem::new(filesystem);

        // Check the metadata of the item.
        // Ignore filesystem errors as we can't do anything useful with the error.
        let Some(metadata) = filesystem.metadata(Path::new(&want_item)).ok() else {
            return Ok(false);
        };

        // Only check directories and files.
        if !(metadata.is_directory() || metadata.is_regular_file()) {
            return Ok(false);
        }
    }

    Ok(true)
}

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

    // Parse the partition uuid from the extractor.
    let partition_uuid = extractor
        .has_partition_uuid
        .as_deref()
        .map(Guid::from_str)
        .transpose()
        .map_err(|e| anyhow!("unable to parse has-partition-uuid: {}", e))?;

    // Parse the partition type uuid from the extractor.
    let partition_type_uuid = extractor
        .has_partition_type_uuid
        .as_deref()
        .map(Guid::from_str)
        .transpose()
        .map_err(|e| anyhow!("unable to parse has-partition-type-uuid: {}", e))?;

    // Find all the filesystems inside the UEFI stack.
    let handles = uefi::boot::find_handles::<SimpleFileSystem>()
        .context("unable to find filesystem handles")?;

    // Iterate over all the filesystems and check if they match the criteria.
    for handle in handles {
        // A filesystem that can't be checked is skipped, as it should not prevent
        // finding a match on the other filesystems or using the fallback value.
        match is_match(
            &context,
            extractor,
            handle,
            partition_uuid,
            partition_type_uuid,
        ) {
            Ok(true) => {}
            Ok(false) => continue,
            Err(error) => {
                warn!("skipping filesystem, unable to check it: {:#}", error);
                continue;
            }
        }

        // If we have a match, return the device root path.
        let path = uefi::boot::open_protocol_exclusive::<DevicePath>(handle)
            .context("unable to open filesystem device path")?;
        let path = path.deref();
        // Acquire the device path root as a string.
        return eficore::path::device_path_root(path).context("unable to get device path root");
    }

    // If there is a fallback value, use it at this point.
    if let Some(fallback) = &extractor.fallback {
        return Ok(fallback.clone());
    }

    // Without a fallback, we can't continue, so bail.
    bail!("unable to find matching filesystem")
}

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

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct FilesystemDeviceMatchExtractor {
    #[serde(default, rename = "has-label")]
    pub has_label: Option<String>,
    #[serde(default, rename = "has-item")]
    pub has_item: Option<String>,
    #[serde(default, rename = "has-partition-uuid")]
    pub has_partition_uuid: Option<String>,
    #[serde(default)]
    pub fallback: Option<String>,
}

pub fn extract(
    context: Rc<SproutContext>,
    extractor: &FilesystemDeviceMatchExtractor,
) -> Result<String> {
    let handles = uefi::boot::find_handles::<SimpleFileSystem>()
        .context("failed to find filesystem handles")?;
    for handle in handles {
        let mut has_match = false;

        let partition_uuid = {
            let partition_info = uefi::boot::open_protocol_exclusive::<PartitionInfo>(handle);

            match partition_info {
                Ok(partition_info) => {
                    if let Some(gpt) = partition_info.gpt_partition_entry() {
                        let uuid = gpt.unique_partition_guid;
                        Some(uuid)
                    } else {
                        None
                    }
                }

                Err(error) => {
                    if error.status() == Status::NOT_FOUND || error.status() == Status::UNSUPPORTED
                    {
                        None
                    } else {
                        Err(error).context("failed to open filesystem partition info")?;
                        None
                    }
                }
            }
        };

        if let Some(partition_uuid) = partition_uuid
            && let Some(ref has_partition_uuid) = extractor.has_partition_uuid
        {
            let parsed_uuid = Guid::from_str(has_partition_uuid)
                .map_err(|e| anyhow!("failed to parse has-uuid: {}", e))?;
            if partition_uuid != parsed_uuid {
                continue;
            }
            has_match = true;
        }

        let mut filesystem = uefi::boot::open_protocol_exclusive::<SimpleFileSystem>(handle)
            .context("failed to open filesystem protocol")?;

        if let Some(ref label) = extractor.has_label {
            let want_label = CString16::try_from(context.stamp(label).as_str())
                .context("failed to convert label to CString16")?;
            let mut root = filesystem
                .open_volume()
                .context("failed to open filesystem volume")?;
            let label = root
                .get_boxed_info::<FileSystemVolumeLabel>()
                .context("failed to get filesystem volume label")?;

            if label.volume_label() != want_label {
                continue;
            }
            has_match = true;
        }

        if let Some(ref item) = extractor.has_item {
            let want_item = CString16::try_from(context.stamp(item).as_str())
                .context("failed to convert item to CString16")?;
            let mut filesystem = FileSystem::new(filesystem);
            let metadata = filesystem.metadata(Path::new(&want_item));

            if metadata.is_err() {
                continue;
            }

            let metadata = metadata?;
            if !(metadata.is_directory() || metadata.is_regular_file()) {
                continue;
            }
            has_match = true;
        }

        if !has_match {
            continue;
        }

        let path = uefi::boot::open_protocol_exclusive::<DevicePath>(handle)
            .context("failed to open filesystem device path")?;
        let path = path.deref();
        return utils::device_path_root(path).context("failed to get device path root");
    }

    if let Some(fallback) = &extractor.fallback {
        return Ok(fallback.clone());
    }
    bail!("unable to find matching filesystem")
}

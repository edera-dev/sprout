use crate::context::SproutContext;
use crate::utils;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use std::rc::Rc;
use uefi::CString16;
use uefi::fs::{FileSystem, Path};
use uefi::proto::device_path::DevicePath;
use uefi::proto::media::file::{File, FileSystemVolumeLabel};
use uefi::proto::media::fs::SimpleFileSystem;

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct FileSystemExtractorConfiguration {
    pub label: Option<String>,
    pub item: Option<String>,
}

pub fn extract(
    context: Rc<SproutContext>,
    device: &FileSystemExtractorConfiguration,
) -> Result<String> {
    let handles = uefi::boot::find_handles::<SimpleFileSystem>()
        .context("failed to find filesystem handles")?;
    for handle in handles {
        let mut filesystem = uefi::boot::open_protocol_exclusive::<SimpleFileSystem>(handle)
            .context("failed to open filesystem protocol")?;

        if let Some(ref label) = device.label {
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
        }

        if let Some(ref item) = device.item {
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
        }

        let path = uefi::boot::open_protocol_exclusive::<DevicePath>(handle)
            .context("failed to open filesystem device path")?;
        let path = path.deref();
        return utils::device_path_root(path).context("failed to get device path root");
    }
    Ok(String::new())
}

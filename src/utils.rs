use anyhow::{Context, Result};
use uefi::CString16;
use uefi::fs::{FileSystem, Path};
use uefi::proto::device_path::text::{AllowShortcuts, DevicePathFromText, DisplayOnly};
use uefi::proto::device_path::{DevicePath, PoolDevicePath};
use uefi::proto::media::fs::SimpleFileSystem;

pub mod framebuffer;

pub fn text_to_device_path(path: &str) -> Result<PoolDevicePath> {
    let path = CString16::try_from(path).context("unable to convert path to CString16")?;
    let device_path_from_text = uefi::boot::open_protocol_exclusive::<DevicePathFromText>(
        uefi::boot::get_handle_for_protocol::<DevicePathFromText>()
            .context("no device path from text protocol")?,
    )
    .context("unable to open device path from text protocol")?;

    device_path_from_text
        .convert_text_to_device_path(&path)
        .context("unable to convert text to device path")
}

pub fn device_path_root(path: &DevicePath) -> Result<String> {
    let mut path = path
        .node_iter()
        .filter_map(|item| {
            let item = item.to_string(DisplayOnly(false), AllowShortcuts(false));
            if item
                .as_ref()
                .map(|item| item.to_string().contains("("))
                .unwrap_or(false)
            {
                Some(item.unwrap_or_default())
            } else {
                None
            }
        })
        .map(|item| item.to_string())
        .collect::<Vec<_>>()
        .join("/");
    path.push('/');
    Ok(path)
}

pub fn read_file_contents(path: &str) -> Result<Vec<u8>> {
    let fs = uefi::boot::open_protocol_exclusive::<SimpleFileSystem>(
        uefi::boot::get_handle_for_protocol::<SimpleFileSystem>()
            .context("no filesystem protocol")?,
    )
    .context("unable to open filesystem protocol")?;
    let mut fs = FileSystem::new(fs);
    let path = CString16::try_from(path).context("unable to convert path to CString16")?;
    let content = fs.read(Path::new(&path));
    content.context("unable to read file contents")
}

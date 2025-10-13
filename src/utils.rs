use anyhow::{Context, Result};
use std::ops::Deref;
use uefi::fs::{FileSystem, Path};
use uefi::proto::device_path::text::{AllowShortcuts, DevicePathFromText, DisplayOnly};
use uefi::proto::device_path::{DevicePath, PoolDevicePath};
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::{CString16, Handle};

pub mod framebuffer;
pub mod linux_media_initrd;

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

pub fn device_path_subpath(path: &DevicePath) -> Result<String> {
    let path = path
        .node_iter()
        .filter_map(|item| {
            let item = item.to_string(DisplayOnly(false), AllowShortcuts(false));
            if item
                .as_ref()
                .map(|item| item.to_string().contains("("))
                .unwrap_or(false)
            {
                None
            } else {
                Some(item.unwrap_or_default())
            }
        })
        .map(|item| item.to_string())
        .collect::<Vec<_>>()
        .join("\\");
    Ok(path)
}

pub struct ResolvedPath {
    pub root_path: Box<DevicePath>,
    pub sub_path: Box<DevicePath>,
    pub full_path: Box<DevicePath>,
    pub filesystem_handle: Handle,
}

pub fn resolve_path(default_root_path: &DevicePath, input: &str) -> Result<ResolvedPath> {
    let mut path = text_to_device_path(input).context("failed to convert text to path")?;
    let path_has_device = path
        .node_iter()
        .next()
        .map(|it| {
            it.to_string(DisplayOnly(false), AllowShortcuts(false))
                .unwrap_or_default()
        })
        .map(|it| it.to_string().contains("("))
        .unwrap_or(false);
    if !path_has_device {
        let mut input = input.to_string();
        if !input.starts_with("\\") {
            input.insert(0, '\\');
        }
        input.insert_str(
            0,
            device_path_root(default_root_path)
                .context("failed to get loaded image device root")?
                .as_str(),
        );
        path = text_to_device_path(input.as_str()).context("failed to convert text to path")?;
    }

    let path = path.to_boxed();
    let root = device_path_root(path.as_ref()).context("failed to convert root to path")?;
    let root_path = text_to_device_path(root.as_str())
        .context("failed to convert root to path")?
        .to_boxed();
    let mut root_path = root_path.as_ref();
    let handle = uefi::boot::locate_device_path::<SimpleFileSystem>(&mut root_path)
        .context("failed to locate filesystem device path")?;
    let subpath = device_path_subpath(path.deref()).context("failed to get device subpath")?;
    Ok(ResolvedPath {
        root_path: root_path.to_boxed(),
        sub_path: text_to_device_path(subpath.as_str())?.to_boxed(),
        full_path: path,
        filesystem_handle: handle,
    })
}

pub fn read_file_contents(default_root_path: &DevicePath, input: &str) -> Result<Vec<u8>> {
    let resolved = resolve_path(default_root_path, input)?;
    let fs = uefi::boot::open_protocol_exclusive::<SimpleFileSystem>(resolved.filesystem_handle)
        .context("unable to open filesystem protocol")?;
    let mut fs = FileSystem::new(fs);
    let path = resolved
        .sub_path
        .to_string(DisplayOnly(false), AllowShortcuts(false))?;
    let content = fs.read(Path::new(&path));
    content.context("unable to read file contents")
}

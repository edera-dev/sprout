use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use anyhow::{Context, Result};
use core::ops::Deref;
use uefi::fs::{FileSystem, Path};
use uefi::proto::device_path::text::{AllowShortcuts, DevicePathFromText, DisplayOnly};
use uefi::proto::device_path::{
    DevicePath, DevicePathNode, DeviceSubType, DeviceType, PoolDevicePath,
};
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::{CString16, Handle};

/// Represents the components of a resolved path.
pub struct ResolvedPath {
    /// The root path of the resolved path. This is the device itself.
    /// For example, "PciRoot(0x0)/Pci(0x4,0x0)/NVMe(0x1,00-00-00-00-00-00-00-00)/HD(1,MBR,0xBE1AFDFA,0x3F,0xFBFC1)/"
    pub root_path: Box<DevicePath>,
    /// The subpath of the resolved path. This is the path to the file.
    /// For example, "\EFI\BOOT\BOOTX64.efi"
    pub sub_path: Box<DevicePath>,
    /// The full path of the resolved path. This is the safest path to use.
    /// For example, "PciRoot(0x0)/Pci(0x4,0x0)/NVMe(0x1,00-00-00-00-00-00-00-00)/HD(1,MBR,0xBE1AFDFA,0x3F,0xFBFC1)/\EFI\BOOT\BOOTX64.efi"
    pub full_path: Box<DevicePath>,
    /// The handle of the filesystem containing the path.
    /// This can be used to acquire a [SimpleFileSystem] protocol to read the file.
    pub filesystem_handle: Handle,
}

impl ResolvedPath {
    /// Read the file specified by this path into a buffer and return it.
    pub fn read_file(&self) -> Result<Vec<u8>> {
        let fs = uefi::boot::open_protocol_exclusive::<SimpleFileSystem>(self.filesystem_handle)
            .context("unable to open filesystem protocol")?;
        let mut fs = FileSystem::new(fs);
        let path = self
            .sub_path
            .to_string16(DisplayOnly(false), AllowShortcuts(false))?;
        let content = fs.read(Path::new(&path));
        content.context("unable to read file contents")
    }
}

/// Checks if the device path `node` is a file path node, rather than a device node.
fn is_file_path_node(node: &DevicePathNode) -> bool {
    node.device_type() == DeviceType::MEDIA && node.sub_type() == DeviceSubType::MEDIA_FILE_PATH
}

/// Converts the device path `node` to its text form.
fn node_to_string(node: &DevicePathNode) -> Result<String> {
    Ok(node
        .to_string16(DisplayOnly(false), AllowShortcuts(false))
        .context("unable to convert device path node to string")?
        .to_string())
}

/// Parses the input `path` as a [DevicePath].
/// Uses the [DevicePathFromText] protocol exclusively, and will fail if it cannot acquire the protocol.
pub fn text_to_device_path(path: impl AsRef<str>) -> Result<PoolDevicePath> {
    let path = CString16::try_from(path.as_ref()).context("unable to convert path to CString16")?;
    let device_path_from_text = uefi::boot::open_protocol_exclusive::<DevicePathFromText>(
        uefi::boot::get_handle_for_protocol::<DevicePathFromText>()
            .context("no device path from text protocol")?,
    )
    .context("unable to open device path from text protocol")?;

    device_path_from_text
        .convert_text_to_device_path(&path)
        .context("unable to convert text to device path")
}

/// Grabs the root part of the `path`.
/// For example, given "PciRoot(0x0)/Pci(0x4,0x0)/NVMe(0x1,00-00-00-00-00-00-00-00)/HD(1,MBR,0xBE1AFDFA,0x3F,0xFBFC1)/\EFI\BOOT\BOOTX64.efi"
/// it will give "PciRoot(0x0)/Pci(0x4,0x0)/NVMe(0x1,00-00-00-00-00-00-00-00)/HD(1,MBR,0xBE1AFDFA,0x3F,0xFBFC1)"
pub fn device_path_root(path: &DevicePath) -> Result<String> {
    let mut path = path
        .node_iter()
        .filter(|node| !is_file_path_node(node))
        .map(node_to_string)
        .collect::<Result<Vec<_>>>()?
        .join("/");
    path.push('/');
    Ok(path)
}

/// Grabs the part of the `path` after the root.
/// For example, given "PciRoot(0x0)/Pci(0x4,0x0)/NVMe(0x1,00-00-00-00-00-00-00-00)/HD(1,MBR,0xBE1AFDFA,0x3F,0xFBFC1)/\EFI\BOOT\BOOTX64.efi"
/// it will give "\EFI\BOOT\BOOTX64.efi"
pub fn device_path_subpath(path: &DevicePath) -> Result<String> {
    let path = path
        .node_iter()
        .filter(|node| is_file_path_node(node))
        .map(node_to_string)
        .collect::<Result<Vec<_>>>()?
        .join("\\");
    Ok(path)
}

/// Resolve a path specified by `input` to its various components.
/// Uses `default_root_path` as the base root if one is not specified in the path.
/// Returns [ResolvedPath] which contains the resolved components.
pub fn resolve_path(
    default_root_path: Option<&DevicePath>,
    input: impl ToString,
) -> Result<ResolvedPath> {
    let mut input = input.to_string();

    let mut path = text_to_device_path(&input).context("unable to convert text to path")?;
    // A path that starts with a device node has a device, while a path that starts
    // with a file path node is relative to the default root path.
    let path_has_device = path
        .node_iter()
        .next()
        .is_some_and(|node| !is_file_path_node(node));
    if !path_has_device {
        if !input.starts_with('\\') {
            input.insert(0, '\\');
        }

        let default_root_path = default_root_path.context("unable to get default root path")?;

        input.insert_str(
            0,
            device_path_root(default_root_path)
                .context("unable to get loaded image device root")?
                .as_str(),
        );
        path = text_to_device_path(input.as_str()).context("unable to convert text to path")?;
    }

    let path = path.to_boxed();
    let root = device_path_root(path.as_ref()).context("unable to convert root to path")?;
    let root_path = text_to_device_path(root.as_str())
        .context("unable to convert root to path")?
        .to_boxed();
    let root_path = root_path.as_ref();

    // locate_device_path modifies the path, so we need to clone it.
    let root_path_modifiable = root_path.to_owned();
    let handle = uefi::boot::locate_device_path::<SimpleFileSystem>(&mut &*root_path_modifiable)
        .context("unable to locate filesystem device path")?;
    let subpath = device_path_subpath(path.deref()).context("unable to get device subpath")?;
    Ok(ResolvedPath {
        root_path: root_path.to_boxed(),
        sub_path: text_to_device_path(subpath.as_str())?.to_boxed(),
        full_path: path,
        filesystem_handle: handle,
    })
}

/// Read the contents of a file at the location specified with the `input` path.
/// Internally, this uses [resolve_path] to resolve the path to its various components.
/// [resolve_path] is passed the `default_root_path` which should specify a base root.
///
/// This acquires exclusive protocol access to the [SimpleFileSystem] protocol of the resolved
/// filesystem handle, so care must be taken to call this function outside a scope with
/// the filesystem handle protocol acquired.
pub fn read_file_contents(default_root_path: Option<&DevicePath>, input: &str) -> Result<Vec<u8>> {
    let resolved = resolve_path(default_root_path, input)?;
    resolved.read_file()
}

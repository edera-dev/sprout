use anyhow::{Context, Result};
use std::ops::Deref;
use uefi::boot::SearchType;
use uefi::fs::{FileSystem, Path};
use uefi::proto::device_path::text::{AllowShortcuts, DevicePathFromText, DisplayOnly};
use uefi::proto::device_path::{DevicePath, PoolDevicePath};
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::proto::media::partition::PartitionInfo;
use uefi::{CString16, Guid, Handle};
use uefi_raw::Status;

/// Support code for the EFI framebuffer.
pub mod framebuffer;

/// Support code for the media loader protocol.
pub mod media_loader;

/// Support code for EFI variables.
pub mod variables;

/// Parses the input `path` as a [DevicePath].
/// Uses the [DevicePathFromText] protocol exclusively, and will fail if it cannot acquire the protocol.
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

/// Checks if a [CString16] contains a char `c`.
/// We need to call to_string() because CString16 doesn't support `contains` with a char.
fn cstring16_contains_char(string: &CString16, c: char) -> bool {
    string.to_string().contains(c)
}

/// Grabs the root part of the `path`.
/// For example, given "PciRoot(0x0)/Pci(0x4,0x0)/NVMe(0x1,00-00-00-00-00-00-00-00)/HD(1,MBR,0xBE1AFDFA,0x3F,0xFBFC1)/\EFI\BOOT\BOOTX64.efi"
/// it will give "PciRoot(0x0)/Pci(0x4,0x0)/NVMe(0x1,00-00-00-00-00-00-00-00)/HD(1,MBR,0xBE1AFDFA,0x3F,0xFBFC1)"
pub fn device_path_root(path: &DevicePath) -> Result<String> {
    let mut path = path
        .node_iter()
        .filter_map(|item| {
            let item = item.to_string(DisplayOnly(false), AllowShortcuts(false));
            if item
                .as_ref()
                .map(|item| cstring16_contains_char(item, '('))
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

/// Grabs the part of the `path` after the root.
/// For example, given "PciRoot(0x0)/Pci(0x4,0x0)/NVMe(0x1,00-00-00-00-00-00-00-00)/HD(1,MBR,0xBE1AFDFA,0x3F,0xFBFC1)/\EFI\BOOT\BOOTX64.efi"
/// it will give "\EFI\BOOT\BOOTX64.efi"
pub fn device_path_subpath(path: &DevicePath) -> Result<String> {
    let path = path
        .node_iter()
        .filter_map(|item| {
            let item = item.to_string(DisplayOnly(false), AllowShortcuts(false));
            if item
                .as_ref()
                .map(|item| cstring16_contains_char(item, '('))
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
            .to_string(DisplayOnly(false), AllowShortcuts(false))?;
        let content = fs.read(Path::new(&path));
        content.context("unable to read file contents")
    }
}

/// Resolve a path specified by `input` to its various components.
/// Uses `default_root_path` as the base root if one is not specified in the path.
/// Returns [ResolvedPath] which contains the resolved components.
pub fn resolve_path(default_root_path: Option<&DevicePath>, input: &str) -> Result<ResolvedPath> {
    let mut path = text_to_device_path(input).context("unable to convert text to path")?;
    let path_has_device = path
        .node_iter()
        .next()
        .map(|it| {
            it.to_string(DisplayOnly(false), AllowShortcuts(false))
                .unwrap_or_default()
        })
        .map(|it| it.to_string().contains('('))
        .unwrap_or(false);
    if !path_has_device {
        let mut input = input.to_string();
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
    let mut root_path = root_path.as_ref();
    let handle = uefi::boot::locate_device_path::<SimpleFileSystem>(&mut root_path)
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

/// Filter a string-like Option `input` such that an empty string is [None].
pub fn empty_is_none<T: AsRef<str>>(input: Option<T>) -> Option<T> {
    input.filter(|input| !input.as_ref().is_empty())
}

/// Combine a sequence of strings into a single string, separated by spaces, ignoring empty strings.
pub fn combine_options<T: AsRef<str>>(options: impl Iterator<Item = T>) -> String {
    options
        .flat_map(|item| empty_is_none(Some(item)))
        .map(|item| item.as_ref().to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Produce a unique hash for the input.
/// This uses SHA-256, which is unique enough but relatively short.
pub fn unique_hash(input: &str) -> String {
    sha256::digest(input.as_bytes())
}

/// Represents the type of partition GUID that can be retrieved.
#[derive(PartialEq, Eq)]
pub enum PartitionGuidForm {
    Partition,
    PartitionType,
}

/// Retrieve the partition / partition type GUID of the device root `path`.
/// This only works on GPT partitions. If the root is not a GPT partition, None is returned.
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
            }))
    } else {
        Ok(None)
    }
}

/// Find a handle that provides the specified `protocol`.
pub fn find_handle(protocol: &Guid) -> Result<Option<Handle>> {
    // Locate the requested protocol handle.
    match uefi::boot::locate_handle_buffer(SearchType::ByProtocol(protocol)) {
        // If a handle is found, the protocol is available.
        Ok(handles) => Ok(if handles.is_empty() {
            None
        } else {
            Some(handles[0])
        }),
        // If an error occurs, check if it is because the protocol is not available.
        // If so, return false. Otherwise, return the error.
        Err(error) => {
            if error.status() == Status::NOT_FOUND {
                Ok(None)
            } else {
                Err(error).context("unable to determine if the protocol is available")
            }
        }
    }
}

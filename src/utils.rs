use uefi::CString16;
use uefi::fs::{FileSystem, Path};
use uefi::proto::device_path::text::{AllowShortcuts, DevicePathFromText, DisplayOnly};
use uefi::proto::device_path::{DevicePath, PoolDevicePath};
use uefi::proto::media::fs::SimpleFileSystem;

pub mod framebuffer;

pub fn text_to_device_path(path: &str) -> PoolDevicePath {
    let path = CString16::try_from(path).expect("unable to convert path to CString16");
    let device_path_from_text = uefi::boot::open_protocol_exclusive::<DevicePathFromText>(
        uefi::boot::get_handle_for_protocol::<DevicePathFromText>()
            .expect("no device path from text protocol"),
    )
    .expect("unable to open device path from text protocol");

    device_path_from_text
        .convert_text_to_device_path(&path)
        .expect("unable to convert text to device path")
}

pub fn device_path_root(path: &DevicePath) -> String {
    let mut path = path
        .node_iter()
        .filter_map(|item| {
            let item = item
                .to_string(DisplayOnly(false), AllowShortcuts(false))
                .expect("unable to convert device path to string");
            if item.to_string().contains("(") {
                Some(item)
            } else {
                None
            }
        })
        .map(|item| item.to_string())
        .collect::<Vec<_>>()
        .join("/");
    path.push('/');
    path
}

pub fn read_file_contents(path: &str) -> Vec<u8> {
    let fs = uefi::boot::open_protocol_exclusive::<SimpleFileSystem>(
        uefi::boot::get_handle_for_protocol::<SimpleFileSystem>().expect("no filesystem protocol"),
    )
    .expect("unable to open filesystem protocol");
    let mut fs = FileSystem::new(fs);
    let path = CString16::try_from(path).expect("unable to convert path to CString16");
    let content = fs.read(Path::new(&path));
    content.expect("unable to read file contents")
}

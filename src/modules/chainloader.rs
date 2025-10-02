use crate::config::ChainloaderConfiguration;
use uefi::{
    CString16,
    proto::device_path::{
        DevicePath, LoadedImageDevicePath, PoolDevicePath,
        text::{AllowShortcuts, DevicePathFromText, DisplayOnly},
    },
};

fn text_to_device_path(path: &str) -> PoolDevicePath {
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

pub fn chainloader(configuration: ChainloaderConfiguration) {
    let sprout_image = uefi::boot::image_handle();
    let image_device_path_protocol =
        uefi::boot::open_protocol_exclusive::<LoadedImageDevicePath>(sprout_image)
            .expect("unable to open loaded image protocol");

    let image_device_path: &DevicePath = &image_device_path_protocol;
    let mut full_path = image_device_path
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
    full_path.push('/');
    full_path.push_str(&configuration.path);

    println!("chainloader: path={}", full_path);

    let device_path = text_to_device_path(&full_path);

    let image = uefi::boot::load_image(
        sprout_image,
        uefi::boot::LoadImageSource::FromDevicePath {
            device_path: &device_path,
            boot_policy: uefi::proto::BootPolicy::ExactMatch,
        },
    )
    .expect("failed to load image");
    uefi::boot::start_image(image).expect("failed to start image");
}

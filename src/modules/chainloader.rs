use crate::config::ChainloaderConfiguration;
use crate::utils::text_to_device_path;
use log::info;
use uefi::proto::device_path::{
    DevicePath, LoadedImageDevicePath,
    text::{AllowShortcuts, DisplayOnly},
};
use uefi::proto::loaded_image::LoadedImage;

pub fn chainloader(configuration: ChainloaderConfiguration) {
    let sprout_image = uefi::boot::image_handle();
    let image_device_path_protocol =
        uefi::boot::open_protocol_exclusive::<LoadedImageDevicePath>(sprout_image)
            .expect("unable to open loaded image device path protocol");

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

    info!("path={}", full_path);

    let device_path = text_to_device_path(&full_path);

    let image = uefi::boot::load_image(
        sprout_image,
        uefi::boot::LoadImageSource::FromDevicePath {
            device_path: &device_path,
            boot_policy: uefi::proto::BootPolicy::ExactMatch,
        },
    )
    .expect("failed to load image");

    let image_device_path_protocol = uefi::boot::open_protocol_exclusive::<LoadedImage>(image)
        .expect("unable to open loaded image protocol");

    let (base, size) = image_device_path_protocol.info();
    info!("loaded image base={:#x} size={:#x}", base.addr(), size);
    uefi::boot::start_image(image).expect("failed to start image");
}

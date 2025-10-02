use crate::config::ChainloaderConfiguration;
use crate::utils;
use log::info;
use uefi::CString16;
use uefi::proto::device_path::LoadedImageDevicePath;
use uefi::proto::loaded_image::LoadedImage;

pub fn chainloader(configuration: ChainloaderConfiguration) {
    let sprout_image = uefi::boot::image_handle();
    let image_device_path_protocol =
        uefi::boot::open_protocol_exclusive::<LoadedImageDevicePath>(sprout_image)
            .expect("unable to open loaded image device path protocol");

    let mut full_path = utils::device_path_root(&image_device_path_protocol);
    full_path.push_str(&configuration.path);

    info!("path={}", full_path);

    let device_path = utils::text_to_device_path(&full_path);

    let image = uefi::boot::load_image(
        sprout_image,
        uefi::boot::LoadImageSource::FromDevicePath {
            device_path: &device_path,
            boot_policy: uefi::proto::BootPolicy::ExactMatch,
        },
    )
    .expect("failed to load image");

    let mut loaded_image_protocol = uefi::boot::open_protocol_exclusive::<LoadedImage>(image)
        .expect("unable to open loaded image protocol");

    let options = configuration.options.join(" ");
    if !options.is_empty() {
        let options = Box::new(
            CString16::try_from(&options[..])
                .expect("unable to convert chainloader options to CString16"),
        );
        info!("options={}", options);

        if options.num_bytes() > u32::MAX as usize {
            panic!("chainloader options too large");
        }

        // SAFETY: options size is checked to validate it is safe to pass.
        // Additionally, the pointer is allocated and retained on the heap which makes
        // passing the options pointer safe to the next image.
        unsafe {
            loaded_image_protocol
                .set_load_options(options.as_ptr() as *const u8, options.num_bytes() as u32);
        }
    }

    let (base, size) = loaded_image_protocol.info();
    info!("loaded image base={:#x} size={:#x}", base.addr(), size);
    uefi::boot::start_image(image).expect("failed to start image");
}

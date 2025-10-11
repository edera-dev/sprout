use crate::context::SproutContext;
use crate::utils;
use anyhow::{Context, Result, bail};
use log::info;
use serde::{Deserialize, Serialize};
use std::rc::Rc;
use uefi::CString16;
use uefi::proto::device_path::LoadedImageDevicePath;
use uefi::proto::loaded_image::LoadedImage;

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct ChainloadConfiguration {
    pub path: String,
    #[serde(default)]
    pub options: Vec<String>,
}

pub fn chainload(context: Rc<SproutContext>, configuration: &ChainloadConfiguration) -> Result<()> {
    let sprout_image = uefi::boot::image_handle();
    let image_device_path_protocol =
        uefi::boot::open_protocol_exclusive::<LoadedImageDevicePath>(sprout_image)
            .context("unable to open loaded image device path protocol")?;

    let mut full_path = utils::device_path_root(&image_device_path_protocol)?;

    full_path.push_str(&context.stamp(&configuration.path));

    info!("path={}", full_path);

    let device_path = utils::text_to_device_path(&full_path)?;

    let image = uefi::boot::load_image(
        sprout_image,
        uefi::boot::LoadImageSource::FromDevicePath {
            device_path: &device_path,
            boot_policy: uefi::proto::BootPolicy::ExactMatch,
        },
    )
    .context("failed to load image")?;

    let mut loaded_image_protocol = uefi::boot::open_protocol_exclusive::<LoadedImage>(image)
        .context("unable to open loaded image protocol")?;

    let options = configuration
        .options
        .iter()
        .map(|item| context.stamp(item))
        .collect::<Vec<_>>()
        .join(" ");
    if !options.is_empty() {
        let options = Box::new(
            CString16::try_from(&options[..])
                .context("unable to convert chainloader options to CString16")?,
        );
        info!("options={}", options);

        if options.num_bytes() > u32::MAX as usize {
            bail!("chainloader options too large");
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
    uefi::boot::start_image(image).context("failed to start image")?;
    Ok(())
}

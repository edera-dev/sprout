use crate::context::SproutContext;
use crate::utils;
use crate::utils::media_loader::MediaLoaderHandle;
use crate::utils::media_loader::constants::linux::LINUX_EFI_INITRD_MEDIA_GUID;
use anyhow::{Context, Result, bail};
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::rc::Rc;
use uefi::CString16;
use uefi::proto::loaded_image::LoadedImage;

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct ChainloadConfiguration {
    pub path: String,
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(default, rename = "linux-initrd")]
    pub linux_initrd: Option<String>,
}

pub fn chainload(context: Rc<SproutContext>, configuration: &ChainloadConfiguration) -> Result<()> {
    let sprout_image = uefi::boot::image_handle();

    let resolved = utils::resolve_path(
        context.root().loaded_image_path()?,
        &context.stamp(&configuration.path),
    )
    .context("unable to resolve chainload path")?;

    let image = uefi::boot::load_image(
        sprout_image,
        uefi::boot::LoadImageSource::FromDevicePath {
            device_path: &resolved.full_path,
            boot_policy: uefi::proto::BootPolicy::ExactMatch,
        },
    )
    .context("unable to load image")?;

    let mut loaded_image_protocol = uefi::boot::open_protocol_exclusive::<LoadedImage>(image)
        .context("unable to open loaded image protocol")?;

    let options = configuration
        .options
        .iter()
        .map(|item| context.stamp(item))
        .collect::<Vec<_>>()
        .join(" ");

    let mut options_holder: Option<Box<CString16>> = None;
    if !options.is_empty() {
        let options = Box::new(
            CString16::try_from(&options[..])
                .context("unable to convert chainloader options to CString16")?,
        );

        info!("options: {}", options);

        if options.num_bytes() > u32::MAX as usize {
            bail!("chainloader options too large");
        }

        // SAFETY: option size is checked to validate it is safe to pass.
        // Additionally, the pointer is allocated and retained on heap, which makes
        // passing the `options` pointer safe to the next image.
        unsafe {
            loaded_image_protocol
                .set_load_options(options.as_ptr() as *const u8, options.num_bytes() as u32);
        }
        options_holder = Some(options);
    }

    let mut initrd_handle = None;
    if let Some(ref linux_initrd) = configuration.linux_initrd {
        let initrd_path = context.stamp(linux_initrd);
        let content = utils::read_file_contents(context.root().loaded_image_path()?, &initrd_path)
            .context("unable to read linux initrd")?;
        let handle =
            MediaLoaderHandle::register(LINUX_EFI_INITRD_MEDIA_GUID, content.into_boxed_slice())
                .context("unable to register linux initrd")?;
        initrd_handle = Some(handle);
    }

    let (base, size) = loaded_image_protocol.info();
    info!("loaded image: base={:#x} size={:#x}", base.addr(), size);
    let result = uefi::boot::start_image(image).context("unable to start image");
    if let Some(initrd_handle) = initrd_handle
        && let Err(error) = initrd_handle.unregister()
    {
        error!("unable to unregister linux initrd: {}", error);
    }
    result.context("unable to start image")?;
    drop(options_holder);
    Ok(())
}

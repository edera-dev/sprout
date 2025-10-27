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

/// The configuration of the chainload action.
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct ChainloadConfiguration {
    /// The path to the image to chainload.
    /// This can be a Linux EFI stub (vmlinuz usually) or a standard EFI executable.
    pub path: String,
    /// The options to pass to the image.
    /// The options are concatenated by a space and then passed to the EFI application.
    #[serde(default)]
    pub options: Vec<String>,
    /// An optional path to a Linux initrd.
    /// This uses the [LINUX_EFI_INITRD_MEDIA_GUID] mechanism to load the initrd into the EFI stack.
    /// For Linux, you can also use initrd=\path\to\initrd as an option, but this option is
    /// generally better and safer as it can support additional load options in the future.
    #[serde(default, rename = "linux-initrd")]
    pub linux_initrd: Option<String>,
}

/// Executes the chainload action using the specified `configuration` inside the provided `context`.
pub fn chainload(context: Rc<SproutContext>, configuration: &ChainloadConfiguration) -> Result<()> {
    // Retrieve the current image handle of sprout.
    let sprout_image = uefi::boot::image_handle();

    // Resolve the path to the image to chainload.
    let resolved = utils::resolve_path(
        context.root().loaded_image_path()?,
        &context.stamp(&configuration.path),
    )
    .context("unable to resolve chainload path")?;

    // Load the image to chainload.
    let image = uefi::boot::load_image(
        sprout_image,
        uefi::boot::LoadImageSource::FromDevicePath {
            device_path: &resolved.full_path,
            boot_policy: uefi::proto::BootPolicy::ExactMatch,
        },
    )
    .context("unable to load image")?;

    // Open the LoadedImage protocol of the image to chainload.
    let mut loaded_image_protocol = uefi::boot::open_protocol_exclusive::<LoadedImage>(image)
        .context("unable to open loaded image protocol")?;

    // Stamp and concatenate the options to pass to the image.
    let options = configuration
        .options
        .iter()
        .map(|item| context.stamp(item))
        .collect::<Vec<_>>()
        .join(" ");

    // Pass the options to the image, if any are provided.
    // The holder must drop at the end of this function to ensure the options are not leaked,
    // and the holder here ensures it outlives the if block here, as a pointer has to be
    // passed to the image. This has been hand-validated to be safe.
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

    // Retrieve the base and size of the loaded image to display.
    let (base, size) = loaded_image_protocol.info();
    info!("loaded image: base={:#x} size={:#x}", base.addr(), size);

    // Start the loaded image.
    // This call might return, or it may pass full control to another image that will never return.
    // Capture the result to ensure we can return an error if the image fails to start, but only
    // after the optional initrd has been unregistered.
    let result = uefi::boot::start_image(image).context("unable to start image");

    // Unregister the initrd if it was registered.
    if let Some(initrd_handle) = initrd_handle
        && let Err(error) = initrd_handle.unregister()
    {
        error!("unable to unregister linux initrd: {}", error);
    }

    // Assert there was no error starting the image.
    result.context("unable to start image")?;
    // Explicitly drop the option holder to clarify the lifetime.
    drop(options_holder);

    // Return control to sprout.
    Ok(())
}

use crate::context::SproutContext;
use crate::utils;
use alloc::boxed::Box;
use alloc::rc::Rc;
use anyhow::{Context, Result, bail};
use edera_sprout_config::actions::chainload::ChainloadConfiguration;
use eficore::bootloader_interface::BootloaderInterface;
use eficore::loader::source::ImageSource;
use eficore::loader::{ImageLoadRequest, ImageLoader};
use eficore::media_loader::MediaLoaderHandle;
use eficore::media_loader::constants::linux::LINUX_EFI_INITRD_MEDIA_GUID;
use uefi::CString16;
use uefi::proto::loaded_image::LoadedImage;

/// Executes the chainload action using the specified `configuration` inside the provided `context`.
pub fn chainload(context: Rc<SproutContext>, configuration: &ChainloadConfiguration) -> Result<()> {
    // Retrieve the current image handle of sprout.
    let sprout_image = uefi::boot::image_handle();

    // Resolve the path to the image to chainload.
    let resolved = eficore::path::resolve_path(
        Some(context.root().loaded_image_path()?),
        &context.stamp(&configuration.path),
    )
    .context("unable to resolve chainload path")?;

    // Create a new image load request with the current image and the resolved path.
    let request = ImageLoadRequest::new(sprout_image, ImageSource::ResolvedPath(&resolved));

    // Load the image to chainload using the image loader support module.
    // It will determine if the image needs to be loaded via the shim or can be loaded directly.
    let image = ImageLoader::load(request)?;

    // Open the LoadedImage protocol of the image to chainload.
    let mut loaded_image_protocol =
        uefi::boot::open_protocol_exclusive::<LoadedImage>(*image.handle())
            .context("unable to open loaded image protocol")?;

    // Stamp and combine the options to pass to the image.
    let options =
        utils::combine_options(configuration.options.iter().map(|item| context.stamp(item)));

    // Pass the load options to the image.
    // If no options are provided, the resulting string will be empty.
    // The options are pinned and boxed to ensure that they are valid for the lifetime of this
    // function, which ensures the lifetime of the options for the image runtime.
    let options = Box::pin(
        CString16::try_from(&options[..])
            .context("unable to convert chainloader options to CString16")?,
    );

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

    // Stamp the initrd path, if provided.
    let initrd = configuration
        .linux_initrd
        .as_ref()
        .map(|item| context.stamp(item));
    // The initrd can be None or empty, so we need to collapse that into a single Option.
    let initrd = utils::empty_is_none(initrd);

    // If an initrd is provided, register it with the EFI stack.
    let mut initrd_handle = None;
    if let Some(linux_initrd) = initrd {
        let content = eficore::path::read_file_contents(
            Some(context.root().loaded_image_path()?),
            &linux_initrd,
        )
        .context("unable to read linux initrd")?;
        let handle =
            MediaLoaderHandle::register(LINUX_EFI_INITRD_MEDIA_GUID, content.into_boxed_slice())
                .context("unable to register linux initrd")?;
        initrd_handle = Some(handle);
    }

    // Mark execution of an entry in the bootloader interface.
    BootloaderInterface::mark_exec(context.root().timer())
        .context("unable to mark execution of boot entry in bootloader interface")?;

    // Start the loaded image.
    // This call might return, or it may pass full control to another image that will never return.
    // Capture the result to ensure we can return an error if the image fails to start, but only
    // after the optional initrd has been unregistered.
    let result = uefi::boot::start_image(*image.handle());

    // Assert there was no error starting the image.
    result.context("unable to start image")?;

    // Explicitly drop the options to clarify the lifetime.
    drop(options);

    // Explicitly drop the initrd handle to clarify when it should be unregistered.
    drop(initrd_handle);

    // Return control to sprout.
    Ok(())
}

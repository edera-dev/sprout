use crate::{actions, context::SproutContext};
use alloc::rc::Rc;
use alloc::string::String;
use alloc::{format, vec};
use anyhow::{Context, Result};
use edera_sprout_config::actions::chainload::ChainloadConfiguration;
use edera_sprout_config::actions::edera::EderaConfiguration;
use edera_sprout_parsing::{build_xen_config, combine_options};
use eficore::media_loader::{
    MediaLoaderHandle,
    constants::xen::{
        XEN_EFI_CONFIG_MEDIA_GUID, XEN_EFI_KERNEL_MEDIA_GUID, XEN_EFI_RAMDISK_MEDIA_GUID,
    },
};
use uefi::Guid;

/// Builds a configuration string for the Xen EFI stub using the specified `configuration`.
fn make_xen_config(context: Rc<SproutContext>, configuration: &EderaConfiguration) -> String {
    let xen_options = combine_options(context.stamp_iter(configuration.xen_options.iter()));
    let kernel_options = combine_options(context.stamp_iter(configuration.kernel_options.iter()));
    build_xen_config(&xen_options, &kernel_options)
}

/// Register a media loader for some `text` with the vendor `guid`.
/// `what` should indicate some identifying value for error messages
/// like `config` or `kernel`.
/// Provides a [MediaLoaderHandle] that can be used to unregister the media loader.
fn register_media_loader_text(guid: Guid, what: &str, text: String) -> Result<MediaLoaderHandle> {
    MediaLoaderHandle::register(guid, text.as_bytes().to_vec().into_boxed_slice())
        .context(format!("unable to register {} media loader", what)) /*  */
}

/// Register a media loader for the file `path` with the vendor `guid`.
/// `what` should indicate some identifying value for error messages
/// like `config` or `kernel`.
/// Provides a [MediaLoaderHandle] that can be used to unregister the media loader.
fn register_media_loader_file(
    context: &Rc<SproutContext>,
    guid: Guid,
    what: &str,
    path: &str,
) -> Result<MediaLoaderHandle> {
    // Stamp the path to the file.
    let path = context.stamp(path);
    // Read the file contents.
    let content =
        eficore::path::read_file_contents(Some(context.root().loaded_image_path()?), &path)
            .context(format!("unable to read {} file", what))?;
    // Register the media loader.
    let handle = MediaLoaderHandle::register(guid, content.into_boxed_slice())
        .context(format!("unable to register {} media loader", what))?;
    Ok(handle)
}

/// Executes the edera action which will boot the Edera hypervisor with the specified
/// `configuration` and `context`. This action uses Edera-specific Xen EFI stub functionality.
pub fn edera(context: Rc<SproutContext>, configuration: &EderaConfiguration) -> Result<()> {
    // Build the Xen config file content for this configuration.
    let config = make_xen_config(context.clone(), configuration);

    // Register the media loader for the config.
    let config = register_media_loader_text(XEN_EFI_CONFIG_MEDIA_GUID, "config", config)
        .context("unable to register config media loader")?;

    // Register the media loaders for the kernel.
    let kernel = register_media_loader_file(
        &context,
        XEN_EFI_KERNEL_MEDIA_GUID,
        "kernel",
        &configuration.kernel,
    )
    .context("unable to register kernel media loader")?;

    // Create a vector of media loaders to drop them only after this function completes.
    let mut media_loaders = vec![config, kernel];

    // Register each initrd segment, filtering out any empty strings or placeholders.
    for initrd_path in configuration.initrd.iter().filter(|s| !s.is_empty()) {
        let handle = register_media_loader_file(
            &context,
            XEN_EFI_RAMDISK_MEDIA_GUID,
            "initrd",
            initrd_path
        ).context("unable to register initrd segment media loader")?;

        media_loaders.push(handle);
    }

    // Chainload to the Xen EFI stub.
    let result = actions::chainload::chainload(
        context.clone(),
        &ChainloadConfiguration {
            path: configuration.xen.clone(),
            options: vec![],
            initrd: vec![],
        },
    )
    .context("unable to chainload to xen");

    // Explicitly drop the media loaders to clarify when they should be unregistered.
    drop(media_loaders);

    result
}

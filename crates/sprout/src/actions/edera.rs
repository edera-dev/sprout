use std::rc::Rc;

use crate::{
    actions,
    context::SproutContext,
    utils::{
        self,
        media_loader::{
            MediaLoaderHandle,
            constants::xen::{
                XEN_EFI_CONFIG_MEDIA_GUID, XEN_EFI_KERNEL_MEDIA_GUID, XEN_EFI_RAMDISK_MEDIA_GUID,
            },
        },
    },
};
use anyhow::{Context, Result};
use edera_sprout_config::actions::chainload::ChainloadConfiguration;
use edera_sprout_config::actions::edera::EderaConfiguration;
use log::error;
use uefi::Guid;

/// Builds a configuration string for the Xen EFI stub using the specified `configuration`.
fn build_xen_config(context: Rc<SproutContext>, configuration: &EderaConfiguration) -> String {
    // Stamp xen options and combine them.
    let xen_options = utils::combine_options(
        configuration
            .xen_options
            .iter()
            .map(|item| context.stamp(item)),
    );

    // Stamp kernel options and combine them.
    let kernel_options = utils::combine_options(
        configuration
            .kernel_options
            .iter()
            .map(|item| context.stamp(item)),
    );

    // xen config file format is ini-like
    [
        // global section
        "[global]".to_string(),
        // default configuration section
        "default=sprout".to_string(),
        // configuration section for sprout
        "[sprout]".to_string(),
        // xen options
        format!("options={}", xen_options),
        // kernel options, stub replaces the kernel path
        // the kernel is provided via media loader
        format!("kernel=stub {}", kernel_options),
        // required or else the last line will be ignored
        "".to_string(),
    ]
    .join("\n")
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
    let content = utils::read_file_contents(Some(context.root().loaded_image_path()?), &path)
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
    let config = build_xen_config(context.clone(), configuration);

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

    // Create a vector of media loaders to unregister on error.
    let mut media_loaders = vec![config, kernel];

    // Register the initrd if it is provided.
    if let Some(initrd) = utils::empty_is_none(configuration.initrd.as_ref()) {
        let initrd =
            register_media_loader_file(&context, XEN_EFI_RAMDISK_MEDIA_GUID, "initrd", initrd)
                .context("unable to register initrd media loader")?;
        media_loaders.push(initrd);
    }

    // Chainload to the Xen EFI stub.
    let result = actions::chainload::chainload(
        context.clone(),
        &ChainloadConfiguration {
            path: configuration.xen.clone(),
            options: vec![],
            linux_initrd: None,
        },
    )
    .context("unable to chainload to xen");

    // Unregister the media loaders when an error happens.
    for media_loader in media_loaders {
        if let Err(error) = media_loader.unregister() {
            error!("unable to unregister media loader: {}", error);
        }
    }

    result
}

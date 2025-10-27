use std::rc::Rc;

use anyhow::{Context, Result};
use log::error;
use serde::{Deserialize, Serialize};
use uefi::Guid;

use crate::{
    actions::{self, chainload::ChainloadConfiguration},
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

/// The configuration of the edera action which boots the Edera hypervisor.
/// Edera is based on Xen but modified significantly with a Rust stack.
/// Sprout is a component of the Edera stack and provides the boot functionality of Xen.
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct EderaConfiguration {
    /// The path to the Xen hypervisor EFI image.
    pub xen: String,
    /// The path to the kernel to boot for dom0.
    pub kernel: String,
    /// The path to the initrd to load for dom0.
    #[serde(default)]
    pub initrd: Option<String>,
    /// The options to pass to the kernel.
    #[serde(default, rename = "kernel-options")]
    pub kernel_options: Vec<String>,
    /// The options to pass to the Xen hypervisor.
    #[serde(default, rename = "xen-options")]
    pub xen_options: Vec<String>,
}

/// Builds a configuration string for the Xen EFI stub using the specified `configuration`.
fn build_xen_config(configuration: &EderaConfiguration) -> String {
    // xen config file format is ini-like
    [
        // global section
        "[global]".to_string(),
        // default configuration section
        "default=sprout".to_string(),
        // configuration section for sprout
        "[sprout]".to_string(),
        // xen options
        format!("options={}", configuration.xen_options.join(" ")),
        // kernel options, stub replaces the kernel path
        // the kernel is provided via media loader
        format!("kernel=stub {}", configuration.kernel_options.join(" ")),
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
    let content = utils::read_file_contents(context.root().loaded_image_path()?, &path)
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
    let config = build_xen_config(configuration);

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
    if let Some(ref initrd) = configuration.initrd {
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

    // Unregister the media loaders on error.
    for media_loader in media_loaders {
        if let Err(error) = media_loader.unregister() {
            error!("unable to unregister media loader: {}", error);
        }
    }

    result
}

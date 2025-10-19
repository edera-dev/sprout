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
            constants::{
                XEN_EFI_CONFIG_MEDIA_GUID, XEN_EFI_KERNEL_MEDIA_GUID, XEN_EFI_RAMDISK_MEDIA_GUID,
            },
        },
    },
};

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct EderaConfiguration {
    pub xen: String,
    pub kernel: String,
    #[serde(default)]
    pub initrd: Option<String>,
    #[serde(default, rename = "kernel-options")]
    pub kernel_options: Vec<String>,
    #[serde(default, rename = "xen-options")]
    pub xen_options: Vec<String>,
}

fn build_xen_config(configuration: &EderaConfiguration) -> String {
    [
        "[global]".to_string(),
        "default=sprout".to_string(),
        "[sprout]".to_string(),
        format!("options={}", configuration.xen_options.join(" ")),
        format!("kernel=stub {}", configuration.kernel_options.join(" ")),
        "".to_string(), // required or else the last line will be ignored
    ]
    .join("\n")
}

fn register_media_loader_text(guid: Guid, what: &str, text: String) -> Result<MediaLoaderHandle> {
    MediaLoaderHandle::register(guid, text.as_bytes().to_vec().into_boxed_slice())
        .context(format!("unable to register {} media loader", what)) /*  */
}

fn register_media_loader_file(
    context: &Rc<SproutContext>,
    guid: Guid,
    what: &str,
    path: &str,
) -> Result<MediaLoaderHandle> {
    let path = context.stamp(path);
    let content = utils::read_file_contents(context.root().loaded_image_path()?, &path)
        .context(format!("unable to read {} file", what))?;
    let handle = MediaLoaderHandle::register(guid, content.into_boxed_slice())
        .context(format!("unable to register {} media loader", what))?;
    Ok(handle)
}

pub fn edera(context: Rc<SproutContext>, configuration: &EderaConfiguration) -> Result<()> {
    let config = build_xen_config(configuration);
    let config = register_media_loader_text(XEN_EFI_CONFIG_MEDIA_GUID, "config", config)
        .context("unable to register config media loader")?;
    let kernel = register_media_loader_file(
        &context,
        XEN_EFI_KERNEL_MEDIA_GUID,
        "kernel",
        &configuration.kernel,
    )
    .context("unable to register kernel media loader")?;

    let mut media_loaders = vec![config, kernel];

    if let Some(ref initrd) = configuration.initrd {
        let initrd =
            register_media_loader_file(&context, XEN_EFI_RAMDISK_MEDIA_GUID, "initrd", initrd)
                .context("unable to register initrd media loader")?;
        media_loaders.push(initrd);
    }

    let result = actions::chainload::chainload(
        context.clone(),
        &ChainloadConfiguration {
            path: configuration.xen.clone(),
            options: vec![],
            linux_initrd: None,
        },
    )
    .context("unable to chainload to xen");

    for media_loader in media_loaders {
        if let Err(error) = media_loader.unregister() {
            error!("unable to unregister media loader: {}", error);
        }
    }

    result
}

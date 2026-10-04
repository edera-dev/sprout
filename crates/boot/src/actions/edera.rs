use crate::{actions, context::SproutContext};
use alloc::rc::Rc;
use alloc::vec::Vec;
use alloc::{format, vec};
use anyhow::{Context, Result, bail};
use edera_sprout_config::actions::chainload::ChainloadConfiguration;
use edera_sprout_config::actions::edera::EderaConfiguration;
use edera_sprout_parsing::{append_initrd, build_xen_config, combine_options, empty_is_none};
use eficore::media_loader::{
    MediaLoaderHandle,
    constants::xen::{
        XEN_EFI_CONFIG_MEDIA_GUID, XEN_EFI_KERNEL_MEDIA_GUID, XEN_EFI_RAMDISK_MEDIA_GUID,
    },
};
use eficore::platform::tpm::PlatformTpm;
use uefi::Guid;

/// Register a media loader for the provided `bytes` with the vendor `guid`.
/// `what` should indicate some identifying value for error messages
/// like `config` or `kernel`.
/// Provides a [MediaLoaderHandle] that can be used to unregister the media loader.
fn register_media_loader_bytes(
    guid: Guid,
    what: &str,
    bytes: Vec<u8>,
) -> Result<MediaLoaderHandle> {
    MediaLoaderHandle::register(guid, bytes.into_boxed_slice())
        .context(format!("unable to register {} media loader", what))
}

/// Read the contents of the loader payload at `path` relative to the sprout image.
/// `what` should indicate some identifying value for error messages
/// like `kernel` or `initrd`.
fn read_loader_payload(context: &Rc<SproutContext>, what: &str, path: &str) -> Result<Vec<u8>> {
    let path = context.stamp(path);
    eficore::path::read_file_contents(Some(context.root().loaded_image_path()?), &path)
        .context(format!("unable to read {} file", what))
}

/// Executes the edera action which will boot the Edera hypervisor with the specified
/// `configuration` and `context`. This action uses Edera-specific Xen EFI stub functionality.
pub fn edera(context: Rc<SproutContext>, configuration: &EderaConfiguration) -> Result<()> {
    // The initrd can be provided as either a single initrd or a chain, but not both.
    if configuration.initrd.is_some() && !configuration.initrd_chain.is_empty() {
        bail!("initrd and initrd-chain cannot be used together");
    }

    // Only register the initrd media loader if the user actually configured one.
    let xen_opts = combine_options(context.stamp_iter(configuration.xen_options.iter()));
    let dom0_args = combine_options(context.stamp_iter(configuration.kernel_options.iter()));

    // Measure xen options and dom0 cmdline into PCR 12 in a fixed order.
    PlatformTpm::log_event(
        PlatformTpm::PCR_KERNEL_PARAMETERS,
        xen_opts.as_bytes(),
        "sprout: xen options",
    )
    .context("unable to measure xen options into the TPM")?;
    PlatformTpm::log_event(
        PlatformTpm::PCR_KERNEL_PARAMETERS,
        dom0_args.as_bytes(),
        "sprout: dom0 cmdline",
    )
    .context("unable to measure dom0 cmdline into the TPM")?;

    // Build the Xen config text and register it as the config media loader. The
    // assembled text is a derived artifact, intentionally not measured.
    let config_handle = register_media_loader_bytes(
        XEN_EFI_CONFIG_MEDIA_GUID,
        "config",
        build_xen_config(&xen_opts, &dom0_args).into_bytes(),
    )
    .context("unable to register config media loader")?;

    // Read the dom0 kernel, measure it into PCR 11, then register the kernel
    // media loader.
    let kernel_bytes = read_loader_payload(&context, "kernel", &configuration.kernel)?;
    PlatformTpm::log_event(
        PlatformTpm::PCR_KERNEL,
        &kernel_bytes,
        "sprout: dom0 kernel",
    )
    .context("unable to measure dom0 kernel into the TPM")?;
    let kernel_handle =
        register_media_loader_bytes(XEN_EFI_KERNEL_MEDIA_GUID, "kernel", kernel_bytes)
            .context("unable to register kernel media loader")?;

    // Read the initrd, or each initrd in the chain concatenated in order, then
    // extend PCR 9 with the combined bytes (empty when no initrd is configured).
    // Paths are stamped first, and paths that are empty after stamping or that refer
    // to the root of a filesystem are skipped, the same as the chainload action.
    let mut initrd_bytes = Vec::new();
    let initrd_paths = configuration
        .initrd
        .iter()
        .chain(configuration.initrd_chain.iter())
        .map(|item| context.stamp(item));
    for path in initrd_paths {
        let Some(path) = empty_is_none(Some(path)) else {
            continue;
        };
        let Some(content) = actions::chainload::read_initrd(&context, &path)
            .context("unable to read initrd file")?
        else {
            continue;
        };
        append_initrd(&mut initrd_bytes, &content);
    }
    PlatformTpm::log_event(
        PlatformTpm::PCR_INITRD,
        &initrd_bytes,
        "sprout: dom0 initrd",
    )
    .context("unable to measure dom0 initrd into the TPM")?;

    // Create a vector of media loaders to drop them only after this function completes.
    let mut media_loaders = vec![config_handle, kernel_handle];

    // Register the initrd if it is provided.
    if !initrd_bytes.is_empty() {
        let initrd_handle =
            register_media_loader_bytes(XEN_EFI_RAMDISK_MEDIA_GUID, "initrd", initrd_bytes)
                .context("unable to register initrd media loader")?;
        media_loaders.push(initrd_handle);
    }

    // Chainload to the Xen EFI stub.
    let result = actions::chainload::chainload(
        context.clone(),
        &ChainloadConfiguration {
            path: configuration.xen.clone(),
            options: vec![],
            linux_initrd: None,
            linux_initrd_chain: vec![],
            devicetree: None,
        },
    )
    .context("unable to chainload to xen");

    // Explicitly drop the media loaders to clarify when they should be unregistered.
    drop(media_loaders);

    result
}

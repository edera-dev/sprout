use crate::context::SproutContext;
use crate::generators::bls::BLS_INITRD_SLOTS;
use crate::phases::before_handoff;
use alloc::boxed::Box;
use alloc::format;
use alloc::rc::Rc;
use alloc::vec::Vec;
use anyhow::{Context, Result, bail};
use edera_sprout_config::actions::chainload::ChainloadConfiguration;
use edera_sprout_parsing::{append_initrd, combine_options, empty_is_none};
use eficore::bootloader_interface::BootloaderInterface;
use eficore::devicetree::DeviceTree;
use eficore::loader::source::ImageSource;
use eficore::loader::{ImageLoadRequest, ImageLoader};
use eficore::media_loader::MediaLoaderHandle;
use eficore::media_loader::constants::linux::LINUX_EFI_INITRD_MEDIA_GUID;
use eficore::path::ResolvedPath;
use eficore::secure::SecureBoot;
use log::warn;
use uefi::proto::loaded_image::LoadedImage;
use uefi::{CString16, Handle};

/// Read the initrd at the stamped `path` relative to the sprout image.
/// Provides [None] if the path refers to the root of a filesystem rather than a file.
pub fn read_initrd(context: &Rc<SproutContext>, path: &str) -> Result<Option<Vec<u8>>> {
    read_optional_file(context, path, "initrd")
}

/// Read the `what` at the stamped `path` relative to the sprout image.
/// Provides [None] if the path refers to the root of a filesystem rather than a file.
fn read_optional_file(
    context: &Rc<SproutContext>,
    path: &str,
    what: &str,
) -> Result<Option<Vec<u8>>> {
    let Some(resolved) = resolve_optional_file(context, path, what)? else {
        return Ok(None);
    };
    let content = resolved
        .read_file()
        .with_context(|| format!("unable to read {}", what))?;
    Ok(Some(content))
}

/// Resolve the `what` at the stamped `path` relative to the sprout image.
/// Provides [None] if the path refers to the root of a filesystem rather than a file.
fn resolve_optional_file(
    context: &Rc<SproutContext>,
    path: &str,
    what: &str,
) -> Result<Option<ResolvedPath>> {
    let resolved = eficore::path::resolve_path(Some(context.root().loaded_image_path()?), path)
        .with_context(|| format!("unable to resolve {} path", what))?;

    // A path without a file component refers to the root of the filesystem, not a file.
    // This happens when a path template like "$root\\$initrd-0" is stamped with an empty
    // value, such as a BLS entry without an initrd or an unused BLS initrd slot.
    let subpath = eficore::path::device_path_subpath(&resolved.full_path)
        .with_context(|| format!("unable to get {} subpath", what))?;
    if subpath.trim_matches('\\').is_empty() {
        return Ok(None);
    }
    Ok(Some(resolved))
}

/// Install the devicetree at the stamped `path` for the image that is about to be started.
/// Provides [None] if there is none to install, such as when the path refers to the root of a
/// filesystem, or when Secure Boot is enabled, as a devicetree can't be verified.
fn install_devicetree(context: &Rc<SproutContext>, path: &str) -> Result<Option<DeviceTree>> {
    let Some(resolved) = resolve_optional_file(context, path, "devicetree")? else {
        return Ok(None);
    };
    if SecureBoot::enabled().unwrap_or(true) {
        warn!("ignoring the devicetree, as Secure Boot is enabled");
        return Ok(None);
    }
    let content = resolved.read_file().context("unable to read devicetree")?;
    let devicetree = DeviceTree::install(&content).context("unable to install the devicetree")?;
    Ok(Some(devicetree))
}

/// Unloads the image with the contained handle when dropped, unless the handle is taken.
/// This ensures that an image that is loaded but never started is not left in memory.
struct UnloadGuard(Option<Handle>);

impl Drop for UnloadGuard {
    fn drop(&mut self) {
        if let Some(handle) = self.0.take()
            && let Err(error) = uefi::boot::unload_image(handle)
        {
            warn!("unable to unload image: {}", error);
        }
    }
}

/// Executes the chainload action using the specified `configuration` inside the provided `context`.
pub fn chainload(context: Rc<SproutContext>, configuration: &ChainloadConfiguration) -> Result<()> {
    // The initrd can be provided as either a single initrd or a chain, but not both.
    if configuration.linux_initrd.is_some() && !configuration.linux_initrd_chain.is_empty() {
        bail!("linux-initrd and linux-initrd-chain cannot be used together");
    }

    // Retrieve the current image handle of sprout.
    let sprout_image = uefi::boot::image_handle();

    // Resolve the path to the image to chainload.
    let resolved = eficore::path::resolve_path(
        Some(context.root().loaded_image_path()?),
        context.stamp(&configuration.path),
    )
    .context("unable to resolve chainload path")?;

    // Create a new image load request with the current image and the resolved path.
    let request = ImageLoadRequest::new(sprout_image, ImageSource::ResolvedPath(&resolved));

    // Load the image to chainload using the image loader support module.
    // It will determine if the image needs to be loaded via the shim or can be loaded directly.
    let image = ImageLoader::load(request)?;

    // Unload the image if an error occurs before it is started.
    let mut unload_guard = UnloadGuard(Some(*image.handle()));

    // Stamp and combine the options to pass to the image.
    let options = combine_options(context.stamp_iter(configuration.options.iter()));

    // Pass the load options to the image.
    // If no options are provided, the resulting string will be empty.
    // The options are pinned and boxed to ensure that they are valid for the lifetime of this
    // function, which ensures the lifetime of the options for the image runtime.
    let options = Box::pin(
        CString16::try_from(&options[..])
            .context("unable to convert chainloader options to CString16")?,
    );

    // Ensure the chainloader options limit is not exceeded.
    if options.num_bytes() > u32::MAX as usize {
        bail!("chainloader options too large");
    }

    // Open the LoadedImage protocol of the image to chainload to pass the load options.
    // This is done in a block to release the protocol before the image is started, as the
    // image may itself need to open its LoadedImage protocol exclusively.
    {
        let mut loaded_image_protocol =
            uefi::boot::open_protocol_exclusive::<LoadedImage>(*image.handle())
                .context("unable to open loaded image protocol")?;

        // SAFETY: option size is checked to validate it is safe to pass.
        // Additionally, the pointer is allocated and retained on heap, which makes
        // passing the `options` pointer safe to the next image.
        unsafe {
            loaded_image_protocol
                .set_load_options(options.as_ptr() as *const u8, options.num_bytes() as u32);
        }
    }

    // Stamp the initrd paths. A single linux-initrd is read the same way as a chain of one.
    let initrd_paths = configuration
        .linux_initrd
        .iter()
        .chain(configuration.linux_initrd_chain.iter())
        .map(|item| context.stamp(item));

    // A BLS entry can have more initrds than the chain lists. Those would be dropped without a
    // word, and then the kernel may not find its root, so say so.
    let chain_length = configuration.linux_initrd_chain.len();
    if chain_length > 0 {
        for slot in chain_length..BLS_INITRD_SLOTS {
            let placeholder = format!("$initrd-{}", slot);
            let value = context.stamp(&placeholder);
            if !value.is_empty() && value != placeholder {
                warn!(
                    "the entry has an initrd for slot {}, but linux-initrd-chain only lists {}, \
                     so it is not loaded",
                    slot, chain_length
                );
                break;
            }
        }
    }

    // Read each initrd and concatenate the contents in order.
    // Paths that are empty after stamping are skipped.
    let mut initrd: Option<Vec<u8>> = None;
    for path in initrd_paths {
        let Some(path) = empty_is_none(Some(path)) else {
            continue;
        };
        let Some(content) = read_initrd(&context, &path)? else {
            continue;
        };
        match initrd.as_mut() {
            Some(initrd) => append_initrd(initrd, &content),
            None => initrd = Some(content),
        }
    }

    // If an initrd was read, register it with the EFI stack.
    // An empty initrd is not registered, as Linux fails to boot if the initrd it is
    // given can't be loaded, instead of booting without one.
    let mut initrd_handle = None;
    if let Some(content) = initrd.filter(|content| !content.is_empty()) {
        let handle =
            MediaLoaderHandle::register(LINUX_EFI_INITRD_MEDIA_GUID, content.into_boxed_slice())
                .context("unable to register linux initrd")?;
        initrd_handle = Some(handle);
    }

    // Install the devicetree, if there is one, for the image to find. It is restored when it is
    // dropped, which is once the image has returned or failed to start. A devicetree is not
    // verified, so it is not used when Secure Boot is enabled, or when that can't be told.
    let mut devicetree = None;
    if let Some(path) = empty_is_none(
        configuration
            .devicetree
            .as_ref()
            .map(|path| context.stamp(path)),
    ) {
        match install_devicetree(&context, &path) {
            Ok(installed) => devicetree = installed,
            // systemd-boot does not boot an entry whose devicetree can't be installed. Outside
            // of strict mode it is booted without it, as before the devicetree was supported.
            Err(error) if !context.root().options().bls_strict_mode => warn!(
                "booting without the devicetree {}, as it can't be installed: {:#} \
                 (systemd-boot and strict mode do not boot the entry)",
                path, error
            ),
            Err(error) => return Err(error),
        }
    }

    // Mark execution of an entry in the bootloader interface.
    // This is only informational, so it should not prevent booting.
    if let Err(error) = BootloaderInterface::mark_exec(context.root().timer()) {
        warn!(
            "unable to mark execution of boot entry in bootloader interface: {:#}",
            error
        );
    }

    // Since we are about to hand off control to another image, we need to execute the handoff hook.
    // This will perform operations like clearing the screen.
    before_handoff(&context).context("unable to execute before handoff hook")?;

    // The image is about to be started, so it is no longer ours to unload.
    // Once started, the image is unloaded by the firmware when it exits.
    unload_guard.0 = None;

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

    // Explicitly drop the devicetree to clarify when the original is restored.
    drop(devicetree);

    // Return control to sprout.
    Ok(())
}

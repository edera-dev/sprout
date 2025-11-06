use crate::loader::source::ImageSource;
use crate::secure::SecureBoot;
use crate::shim::hook::SecurityHook;
use crate::shim::{ShimInput, ShimSupport};
use anyhow::{Context, Result, bail};
use log::warn;
use uefi::Handle;
use uefi::boot::LoadImageSource;

/// Represents EFI image sources generically.
pub mod source;

/// Handle to a loaded EFI image.
pub struct ImageHandle {
    /// Handle to the loaded image.
    handle: Handle,
}

impl ImageHandle {
    /// Create a new image handle based on a handle from the UEFI stack.
    pub fn new(handle: Handle) -> Self {
        Self { handle }
    }

    /// Retrieve the underlying handle.
    pub fn handle(&self) -> &Handle {
        &self.handle
    }
}

/// Request to load an image from a source, with support for additional validation features.
pub struct ImageLoadRequest<'source> {
    /// Handle to the current image.
    current_image: Handle,
    /// Source of the image to load.
    source: ImageSource<'source>,
}

impl<'source> ImageLoadRequest<'source> {
    /// Create a new image load request with a current image and a source.
    pub fn new(current_image: Handle, source: ImageSource<'source>) -> Self {
        Self {
            current_image,
            source,
        }
    }

    /// Retrieve the current image.
    pub fn current_image(&self) -> &Handle {
        &self.current_image
    }

    /// Retrieve the source of the image to load.
    pub fn source(&'source self) -> &'source ImageSource<'source> {
        &self.source
    }

    /// Convert the request into a source.
    pub fn into_source(self) -> ImageSource<'source> {
        self.source
    }
}

/// EFI image loader.
pub struct ImageLoader;

impl ImageLoader {
    /// Load an image using the image `request` which allows
    pub fn load(request: ImageLoadRequest) -> Result<ImageHandle> {
        // Determine whether Secure Boot is enabled.
        let secure_boot =
            SecureBoot::enabled().context("unable to determine if secure boot is enabled")?;

        // Determine whether the shim is loaded.
        let shim_loaded = ShimSupport::loaded().context("unable to determine if shim is loaded")?;

        // Determine whether the shim loader is available.
        let shim_loader_available = ShimSupport::loader_available()
            .context("unable to determine if shim loader is available")?;

        // Determines whether LoadImage in Boot Services must be patched.
        // Version 16 of the shim doesn't require extra effort to load Secure Boot binaries.
        // If the image loader is installed, we can skip over the security hook.
        let requires_security_hook = secure_boot && shim_loaded && !shim_loader_available;

        // If the security hook is required, we will bail for now.
        if requires_security_hook {
            // Install the security hook, if possible. If it's not, this is necessary to continue,
            // so we should bail.
            let installed = SecurityHook::install().context("unable to install security hook")?;
            if !installed {
                bail!("unable to install security hook required for this platform");
            }
        }

        // If the shim is loaded, we will need to retain the shim protocol to allow
        // loading multiple images.
        if shim_loaded {
            // Retain the shim protocol after loading the image.
            ShimSupport::retain()?;
        }

        // Clone the current image handle to use for loading the image.
        let current_image = *request.current_image();

        // Converts the source to a shim input with an owned data buffer.
        let input = ShimInput::from(request.into_source())
            .into_owned_data_buffer()
            .context("unable to convert input to loaded data buffer")?;

        // Constructs a LoadImageSource from the input.
        let source = LoadImageSource::FromBuffer {
            buffer: input.buffer().context("unable to get buffer from input")?,
            file_path: input.file_path(),
        };

        // Loads the image using Boot Services LoadImage function.
        let result = uefi::boot::load_image(current_image, source).context("unable to load image");

        // If the security override is required, we will uninstall the security hook.
        if requires_security_hook {
            let uninstall_result = crate::shim::hook::SecurityHook::uninstall();
            // Ensure we don't mask load image errors if uninstalling fails.
            if result.is_err()
                && let Err(uninstall_error) = &uninstall_result
            {
                // Warn on the error since the load image error is more important.
                warn!("unable to uninstall security hook: {}", uninstall_error);
            } else {
                // Otherwise, ensure we handle the original uninstallation result.
                uninstall_result?;
            }
        }

        // Assert the result and grab the handle.
        let handle = result?;

        // Retrieve the handle from the result and make a new image handle.
        Ok(ImageHandle::new(handle))
    }
}

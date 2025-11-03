use crate::integrations::shim::hook::SecurityHook;
use crate::secure::SecureBoot;
use crate::utils;
use crate::utils::ResolvedPath;
use crate::utils::variables::{VariableClass, VariableController};
use alloc::boxed::Box;
use alloc::string::ToString;
use alloc::vec::Vec;
use anyhow::{Context, Result, anyhow, bail};
use core::ffi::c_void;
use core::pin::Pin;
use log::warn;
use uefi::Handle;
use uefi::boot::LoadImageSource;
use uefi::proto::device_path::text::{AllowShortcuts, DisplayOnly};
use uefi::proto::device_path::{DevicePath, FfiDevicePath};
use uefi::proto::unsafe_protocol;
use uefi_raw::table::runtime::VariableVendor;
use uefi_raw::{Guid, Status, guid};

/// Security hook support.
mod hook;

/// Support for the shim loader application for Secure Boot.
pub struct ShimSupport;

/// Input to the shim mechanisms.
pub enum ShimInput<'a> {
    /// Data loaded into a buffer and ready to be verified, owned.
    OwnedDataBuffer(Option<&'a ResolvedPath>, Pin<Box<[u8]>>),
    /// Data loaded into a buffer and ready to be verified.
    DataBuffer(Option<&'a ResolvedPath>, &'a [u8]),
    /// Low-level data buffer provided by the security hook.
    SecurityHookBuffer(Option<*const FfiDevicePath>, &'a [u8]),
    /// Low-level owned data buffer provided by the security hook.
    SecurityHookOwnedBuffer(Option<*const FfiDevicePath>, Pin<Box<[u8]>>),
    /// Low-level path provided by the security hook.
    SecurityHookPath(*const FfiDevicePath),
    /// Data is provided as a resolved path. We will need to load the data to verify it.
    /// The output will them return the loaded data.
    ResolvedPath(&'a ResolvedPath),
}

impl<'a> ShimInput<'a> {
    /// Accesses the buffer behind the shim input, if available.
    pub fn buffer(&self) -> Option<&[u8]> {
        match self {
            ShimInput::OwnedDataBuffer(_, data) => Some(data),
            ShimInput::SecurityHookOwnedBuffer(_, data) => Some(data),
            ShimInput::SecurityHookBuffer(_, data) => Some(data),
            ShimInput::SecurityHookPath(_) => None,
            ShimInput::DataBuffer(_, data) => Some(data),
            ShimInput::ResolvedPath(_) => None,
        }
    }

    /// Accesses the full device path to the input.
    pub fn file_path(&self) -> Option<&DevicePath> {
        match self {
            ShimInput::OwnedDataBuffer(path, _) => path.as_ref().map(|it| it.full_path.as_ref()),
            ShimInput::DataBuffer(path, _) => path.as_ref().map(|it| it.full_path.as_ref()),
            ShimInput::SecurityHookBuffer(path, _) => {
                path.map(|it| unsafe { DevicePath::from_ffi_ptr(it) })
            }
            ShimInput::SecurityHookPath(path) => unsafe { Some(DevicePath::from_ffi_ptr(*path)) },
            ShimInput::ResolvedPath(path) => Some(path.full_path.as_ref()),
            ShimInput::SecurityHookOwnedBuffer(path, _) => {
                path.map(|it| unsafe { DevicePath::from_ffi_ptr(it) })
            }
        }
    }

    /// Converts this input into an owned data buffer, where the data is loaded.
    /// For ResolvedPath, this will read the file.
    pub fn into_owned_data_buffer(self) -> Result<ShimInput<'a>> {
        match self {
            ShimInput::OwnedDataBuffer(root, data) => Ok(ShimInput::OwnedDataBuffer(root, data)),

            ShimInput::DataBuffer(root, data) => Ok(ShimInput::OwnedDataBuffer(
                root,
                Box::into_pin(data.to_vec().into_boxed_slice()),
            )),

            ShimInput::SecurityHookPath(ffi_path) => {
                // Acquire the file path.
                let Some(path) = self.file_path() else {
                    bail!("unable to convert security hook path to device path");
                };
                // Convert the underlying path to a string.
                let path = path
                    .to_string(DisplayOnly(false), AllowShortcuts(false))
                    .context("unable to convert device path to string")?;
                let path = utils::resolve_path(None, &path.to_string())
                    .context("unable to resolve path")?;
                // Read the file path.
                let data = path.read_file()?;
                Ok(ShimInput::SecurityHookOwnedBuffer(
                    Some(ffi_path),
                    Box::into_pin(data.to_vec().into_boxed_slice()),
                ))
            }

            ShimInput::SecurityHookBuffer(_, _) => {
                bail!("unable to convert security hook buffer to owned data buffer")
            }

            ShimInput::ResolvedPath(path) => {
                // Read the file path.
                let data = path.read_file()?;
                Ok(ShimInput::OwnedDataBuffer(
                    Some(path),
                    Box::into_pin(data.to_vec().into_boxed_slice()),
                ))
            }

            ShimInput::SecurityHookOwnedBuffer(path, data) => {
                Ok(ShimInput::SecurityHookOwnedBuffer(path, data))
            }
        }
    }
}

/// Output of the shim verification function.
/// Since the shim needs to load the data from disk, we will optimize by using that as the data
/// to actually boot.
pub enum ShimVerificationOutput {
    /// The verification failed.
    VerificationFailed(Status),
    /// The data provided to the verifier was already a buffer.
    VerifiedDataNotLoaded,
    /// Verifying the data resulted in loading the data from the source.
    /// This contains the data that was loaded, so it won't need to be loaded again.
    VerifiedDataBuffer(Vec<u8>),
}

/// The shim lock protocol as defined by the shim loader application.
#[unsafe_protocol(ShimSupport::SHIM_LOCK_GUID)]
struct ShimLockProtocol {
    /// Verify the data in `buffer` with the size `buffer_size` to determine if it is valid.
    /// NOTE: On x86_64, this function uses SYSV calling conventions. On aarch64 it uses the
    /// efiapi calling convention. This is truly wild, but you can verify it yourself by
    /// looking at: https://github.com/rhboot/shim/blob/15.8/shim.h#L207-L212
    /// There is no calling convention declared like there should be.
    #[cfg(target_arch = "x86_64")]
    pub shim_verify: unsafe extern "sysv64" fn(buffer: *const c_void, buffer_size: u32) -> Status,
    #[cfg(target_arch = "aarch64")]
    pub shim_verify: unsafe extern "efiapi" fn(buffer: *const c_void, buffer_size: u32) -> Status,
    /// Unused function that is defined by the shim.
    _generate_header: *mut c_void,
    /// Unused function that is defined by the shim.
    _read_header: *mut c_void,
}

impl ShimSupport {
    /// Variable controller for the shim lock.
    const SHIM_LOCK_VARIABLES: VariableController =
        VariableController::new(VariableVendor(Self::SHIM_LOCK_GUID));

    /// GUID for the shim lock protocol.
    const SHIM_LOCK_GUID: Guid = guid!("605dab50-e046-4300-abb6-3dd810dd8b23");
    /// GUID for the shim image loader protocol.
    const SHIM_IMAGE_LOADER_GUID: Guid = guid!("1f492041-fadb-4e59-9e57-7cafe73a55ab");

    /// Determines whether the shim is loaded.
    pub fn loaded() -> Result<bool> {
        Ok(utils::find_handle(&Self::SHIM_LOCK_GUID)
            .context("unable to find shim lock protocol")?
            .is_some())
    }

    /// Determines whether the shim loader is available.
    pub fn loader_available() -> Result<bool> {
        Ok(utils::find_handle(&Self::SHIM_IMAGE_LOADER_GUID)
            .context("unable to find shim image loader protocol")?
            .is_some())
    }

    /// Use the shim to validate the `input`, returning [ShimVerificationOutput] when complete.
    pub fn verify(input: ShimInput) -> Result<ShimVerificationOutput> {
        // Acquire the handle to the shim lock protocol.
        let handle = utils::find_handle(&Self::SHIM_LOCK_GUID)
            .context("unable to find shim lock protocol")?
            .ok_or_else(|| anyhow!("unable to find shim lock protocol"))?;
        // Acquire the protocol exclusively to the shim lock.
        let protocol = uefi::boot::open_protocol_exclusive::<ShimLockProtocol>(handle)
            .context("unable to open shim lock protocol")?;

        // If the input type is a device path, we need to load the data.
        let maybe_loaded_data = match input {
            ShimInput::OwnedDataBuffer(_, _data) => {
                bail!("owned data buffer is not supported in the verification function");
            }
            ShimInput::SecurityHookBuffer(_, _) => None,
            ShimInput::SecurityHookOwnedBuffer(_, _) => None,
            ShimInput::DataBuffer(_, _) => None,
            ShimInput::ResolvedPath(path) => Some(path.read_file()?),
            ShimInput::SecurityHookPath(_) => None,
        };

        // Convert the input to a buffer.
        // If the input provides the data buffer, we will use that.
        // Otherwise, we will use the data loaded by this function.
        let buffer = match &input {
            ShimInput::OwnedDataBuffer(_root, data) => data,
            ShimInput::DataBuffer(_root, data) => *data,
            ShimInput::ResolvedPath(_path) => maybe_loaded_data
                .as_deref()
                .context("expected data buffer to be loaded already")?,
            ShimInput::SecurityHookBuffer(_, data) => data,
            ShimInput::SecurityHookOwnedBuffer(_, data) => data,
            ShimInput::SecurityHookPath(_) => {
                bail!("security hook path input not supported in the verification function")
            }
        };

        // Check if the buffer is too large to verify.
        if buffer.len() > u32::MAX as usize {
            bail!("buffer is too large to verify with shim lock protocol");
        }

        // Call the shim verify function.
        // SAFETY: The shim verify function is specified by the shim lock protocol.
        // Calling this function is considered safe because the shim verify function is
        // guaranteed to be defined by the environment if we are able to acquire the protocol.
        let status = unsafe {
            (protocol.shim_verify)(buffer.as_ptr() as *const c_void, buffer.len() as u32)
        };

        // If the verification failed, return the verification failure output.
        if !status.is_success() {
            return Ok(ShimVerificationOutput::VerificationFailed(status));
        }

        // If verification succeeded, return the validation output,
        // which might include the loaded data.
        Ok(maybe_loaded_data
            .map(ShimVerificationOutput::VerifiedDataBuffer)
            .unwrap_or(ShimVerificationOutput::VerifiedDataNotLoaded))
    }

    /// Load the image specified by the `input` and returns an image handle.
    pub fn load(current_image: Handle, input: ShimInput) -> Result<Handle> {
        // Determine whether Secure Boot is enabled.
        let secure_boot =
            SecureBoot::enabled().context("unable to determine if secure boot is enabled")?;

        // Determine whether the shim is loaded.
        let shim_loaded = Self::loaded().context("unable to determine if shim is loaded")?;

        // Determine whether the shim loader is available.
        let shim_loader_available =
            Self::loader_available().context("unable to determine if shim loader is available")?;

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
            Self::retain()?;
        }

        // Converts the shim input to an owned data buffer.
        let input = input
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
            let uninstall_result = SecurityHook::uninstall();
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
        result
    }

    /// Set the ShimRetainProtocol variable to indicate that shim should retain the protocols
    /// for the full lifetime of boot services.
    pub fn retain() -> Result<()> {
        Self::SHIM_LOCK_VARIABLES
            .set_bool(
                "ShimRetainProtocol",
                true,
                VariableClass::BootAndRuntimeTemporary,
            )
            .context("unable to retain shim protocol")?;
        Ok(())
    }
}

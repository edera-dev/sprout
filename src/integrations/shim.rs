use crate::utils;
use crate::utils::ResolvedPath;
use anyhow::{Context, Result, anyhow, bail};
use std::ffi::c_void;
use uefi::Handle;
use uefi::boot::LoadImageSource;
use uefi::proto::device_path::DevicePath;
use uefi::proto::unsafe_protocol;
use uefi_raw::{Guid, Status, guid};

/// Support for the shim loader application for Secure Boot.
pub struct ShimSupport;

/// Input to the shim mechanisms.
pub enum ShimInput<'a> {
    /// Data loaded into a buffer and ready to be verified, owned.
    OwnedDataBuffer(Option<&'a ResolvedPath>, Vec<u8>),
    /// Data loaded into a buffer and ready to be verified.
    DataBuffer(Option<&'a ResolvedPath>, &'a [u8]),
    /// Data is provided as a resolved path. We will need to load the data to verify it.
    /// The output will them return the loaded data.
    ResolvedPath(&'a ResolvedPath),
}

impl<'a> ShimInput<'a> {
    /// Accesses the buffer behind the shim input, if available.
    pub fn buffer(&self) -> Option<&[u8]> {
        match self {
            ShimInput::OwnedDataBuffer(_, data) => Some(data),
            ShimInput::DataBuffer(_, data) => Some(data),
            ShimInput::ResolvedPath(_) => None,
        }
    }

    /// Accesses the full device path to the input.
    pub fn file_path(&self) -> Option<&DevicePath> {
        match self {
            ShimInput::OwnedDataBuffer(path, _) => path.as_ref().map(|it| it.full_path.as_ref()),
            ShimInput::DataBuffer(path, _) => path.as_ref().map(|it| it.full_path.as_ref()),
            ShimInput::ResolvedPath(path) => Some(path.full_path.as_ref()),
        }
    }

    /// Converts this input into an owned data buffer, where the data is loaded.
    /// For ResolvedPath, this will read the file.
    pub fn into_owned_data_buffer(self) -> Result<ShimInput<'a>> {
        match self {
            ShimInput::OwnedDataBuffer(root, data) => Ok(ShimInput::OwnedDataBuffer(root, data)),

            ShimInput::DataBuffer(root, data) => {
                Ok(ShimInput::OwnedDataBuffer(root, data.to_vec()))
            }

            ShimInput::ResolvedPath(path) => {
                Ok(ShimInput::OwnedDataBuffer(Some(path), path.read_file()?))
            }
        }
    }
}

/// Output of the shim verification function.
/// Since the shim needs to load the data from disk, we will optimize by using that as the data
/// to actually boot.
pub enum ShimVerificationOutput {
    /// The verification failed.
    VerificationFailed,
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
    pub shim_verify: unsafe extern "efiapi" fn(buffer: *mut c_void, buffer_size: u32) -> Status,
    /// Unused function that is defined by the shim.
    _generate_header: *mut c_void,
    /// Unused function that is defined by the shim.
    _read_header: *mut c_void,
}

impl ShimSupport {
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
    pub fn validate(input: ShimInput) -> Result<ShimVerificationOutput> {
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
            ShimInput::DataBuffer(_, _) => None,
            ShimInput::ResolvedPath(path) => Some(path.read_file()?),
        };

        // Convert the input to a buffer.
        // If the input provides the data buffer, we will use that.
        // Otherwise, we will use the data loaded by this function.
        let buffer = match input {
            ShimInput::OwnedDataBuffer(_root, _data) => {
                bail!("owned data buffer is not supported in the verification function");
            }
            ShimInput::DataBuffer(_root, data) => data,
            ShimInput::ResolvedPath(_path) => maybe_loaded_data
                .as_deref()
                .context("expected data buffer to be loaded already")?,
        };

        // Check if the buffer is too large to verify.
        if buffer.len() > u32::MAX as usize {
            bail!("buffer is too large to verify with shim lock protocol");
        }

        // Call the shim verify function.
        // SAFETY: The shim verify function is specified by the shim lock protocol.
        // Calling this function is considered safe because
        let status =
            unsafe { (protocol.shim_verify)(buffer.as_ptr() as *mut c_void, buffer.len() as u32) };

        // If the verification failed, return the verification failure output.
        if !status.is_success() {
            return Ok(ShimVerificationOutput::VerificationFailed);
        }

        // If verification succeeded, return the validation output,
        // which might include the loaded data.
        Ok(maybe_loaded_data
            .map(ShimVerificationOutput::VerifiedDataBuffer)
            .unwrap_or(ShimVerificationOutput::VerifiedDataNotLoaded))
    }

    /// Load the image specified by the `input` and returns an image handle.
    pub fn load(current_image: Handle, input: ShimInput) -> Result<Handle> {
        // Determine whether the shim is loaded.
        let shim_loaded = Self::loaded().context("unable to determine if shim is loaded")?;

        // Determine whether the shim loader is available.
        let shim_loader_available =
            Self::loader_available().context("unable to determine if shim loader is available")?;

        // Determines whether LoadImage in Boot Services must be patched.
        // Version 16 of the shim doesn't require extra effort to load Secure Boot binaries.
        // If the image loader is installed, we can skip over the security override.
        let requires_security_override = shim_loaded && !shim_loader_available;

        // If the security override is required, we will bail for now.
        if requires_security_override {
            bail!("shim image loader protocol is not available, please upgrade to shim version 16");
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
        uefi::boot::load_image(current_image, source).context("unable to load image")
    }
}

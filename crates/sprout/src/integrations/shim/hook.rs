use crate::integrations::shim::{ShimInput, ShimSupport, ShimVerificationOutput};
use crate::utils;
use anyhow::{Context, Result};
use core::slice;
use log::warn;
use spin::{Lazy, Mutex};
use uefi::proto::device_path::FfiDevicePath;
use uefi::proto::unsafe_protocol;
use uefi::{Guid, guid};
use uefi_raw::Status;

/// GUID for the EFI_SECURITY_ARCH protocol.
const SECURITY_ARCH_GUID: Guid = guid!("a46423e3-4617-49f1-b9ff-d1bfa9115839");
/// GUID for the EFI_SECURITY_ARCH2 protocol.
const SECURITY_ARCH2_GUID: Guid = guid!("94ab2f58-1438-4ef1-9152-18941a3a0e68");

/// EFI_SECURITY_ARCH protocol definition.
#[unsafe_protocol(SECURITY_ARCH_GUID)]
pub struct SecurityArchProtocol {
    /// Determines the file authentication state.
    pub file_authentication_state: unsafe extern "efiapi" fn(
        this: *const SecurityArchProtocol,
        status: u32,
        path: *const FfiDevicePath,
    ) -> Status,
}

/// EFI_SECURITY_ARCH2 protocol definition.
#[unsafe_protocol(SECURITY_ARCH2_GUID)]
pub struct SecurityArch2Protocol {
    /// Determines the file authentication.
    pub file_authentication: unsafe extern "efiapi" fn(
        this: *const SecurityArch2Protocol,
        path: *const FfiDevicePath,
        file_buffer: *const u8,
        file_size: usize,
        boot_policy: bool,
    ) -> Status,
}

/// Global state for the security hook.
struct SecurityHookState {
    original_hook: SecurityArchProtocol,
    original_hook2: SecurityArch2Protocol,
}

/// Global state for the security hook.
/// This is messy, but it is safe given the mutex.
static GLOBAL_HOOK_STATE: Lazy<Mutex<Option<SecurityHookState>>> = Lazy::new(|| Mutex::new(None));

/// Security hook helper.
pub struct SecurityHook;

impl SecurityHook {
    /// Shared verifier logic for both hook types.
    #[must_use]
    fn verify(input: ShimInput) -> bool {
        // Verify the input and convert the result to a status.
        let status = match ShimSupport::verify(input) {
            Ok(output) => match output {
                // If the verification failed, return the access-denied status.
                ShimVerificationOutput::VerificationFailed(status) => status,
                // If the verification succeeded, return the success status.
                ShimVerificationOutput::VerifiedDataNotLoaded => Status::SUCCESS,
                ShimVerificationOutput::VerifiedDataBuffer(_) => Status::SUCCESS,
            },

            // If an error occurs, log the error since we can't return a better error.
            // Then return the access-denied status.
            Err(error) => {
                warn!("unable to verify image: {}", error);
                Status::ACCESS_DENIED
            }
        };

        // If the status is not a success, log the status.
        if !status.is_success() {
            warn!("shim verification failed: {}", status);
        }
        // Return whether the status is a success.
        // If it's not a success, the original hook should be called.
        status.is_success()
    }

    /// File authentication state verifier for the EFI_SECURITY_ARCH protocol.
    /// Takes the `path` and determines the verification.
    unsafe extern "efiapi" fn arch_file_authentication_state(
        this: *const SecurityArchProtocol,
        status: u32,
        path: *const FfiDevicePath,
    ) -> Status {
        // Verify the path is not null.
        if path.is_null() {
            return Status::INVALID_PARAMETER;
        }

        // Construct a shim input from the path.
        let input = ShimInput::SecurityHookPath(path);

        // Convert the input to an owned data buffer.
        let input = match input.into_owned_data_buffer() {
            Ok(input) => input,
            // If an error occurs, log the error and return the not found status.
            Err(error) => {
                warn!("unable to read data to be authenticated: {}", error);
                return Status::NOT_FOUND;
            }
        };

        // Verify the input, if it fails, call the original hook.
        if !Self::verify(input) {
            // Acquire the global hook state to grab the original hook.
            let function = match GLOBAL_HOOK_STATE.lock().as_ref() {
                // The hook state is available, so we can acquire the original hook.
                Some(state) => state.original_hook.file_authentication_state,

                // The hook state is not available, so we can't call the original hook.
                None => {
                    warn!("global hook state is not available, unable to call original hook");
                    return Status::LOAD_ERROR;
                }
            };

            // Call the original hook function to see what it reports.
            // SAFETY: This function is safe to call as it is stored by us and is required
            // in the UEFI specification.
            unsafe { function(this, status, path) }
        } else {
            Status::SUCCESS
        }
    }

    /// File authentication verifier for the EFI_SECURITY_ARCH2 protocol.
    /// Takes the `path` and a file buffer to determine the verification.
    unsafe extern "efiapi" fn arch2_file_authentication(
        this: *const SecurityArch2Protocol,
        path: *const FfiDevicePath,
        file_buffer: *const u8,
        file_size: usize,
        boot_policy: bool,
    ) -> Status {
        // Verify the path and file buffer are not null.
        if path.is_null() || file_buffer.is_null() {
            return Status::INVALID_PARAMETER;
        }

        // If the boot policy is true, we can't continue as we don't support that.
        if boot_policy {
            return Status::INVALID_PARAMETER;
        }

        // Construct a slice out of the file buffer and size.
        let buffer = unsafe { slice::from_raw_parts(file_buffer, file_size) };

        // Construct a shim input from the path.
        let input = ShimInput::SecurityHookBuffer(Some(path), buffer);

        // Verify the input, if it fails, call the original hook.
        if !Self::verify(input) {
            // Acquire the global hook state to grab the original hook.
            let function = match GLOBAL_HOOK_STATE.lock().as_ref() {
                // The hook state is available, so we can acquire the original hook.
                Some(state) => state.original_hook2.file_authentication,

                // The hook state is not available, so we can't call the original hook.
                None => {
                    warn!("global hook state is not available, unable to call original hook");
                    return Status::LOAD_ERROR;
                }
            };

            // Call the original hook function to see what it reports.
            // SAFETY: This function is safe to call as it is stored by us and is required
            // in the UEFI specification.
            unsafe { function(this, path, file_buffer, file_size, boot_policy) }
        } else {
            Status::SUCCESS
        }
    }

    /// Install the security hook if needed.
    pub fn install() -> Result<bool> {
        // Find the security arch protocol. If we can't find it, we will return false.
        let Some(hook_arch) = utils::find_handle(&SECURITY_ARCH_GUID)
            .context("unable to check security arch existence")?
        else {
            return Ok(false);
        };

        // Find the security arch2 protocol. If we can't find it, we will return false.
        let Some(hook_arch2) = utils::find_handle(&SECURITY_ARCH2_GUID)
            .context("unable to check security arch2 existence")?
        else {
            return Ok(false);
        };

        // Open the security arch protocol.
        let mut arch_protocol =
            uefi::boot::open_protocol_exclusive::<SecurityArchProtocol>(hook_arch)
                .context("unable to open security arch protocol")?;

        // Open the security arch2 protocol.
        let mut arch_protocol2 =
            uefi::boot::open_protocol_exclusive::<SecurityArch2Protocol>(hook_arch2)
                .context("unable to open security arch2 protocol")?;

        // Construct the global state to store.
        let state = SecurityHookState {
            original_hook: SecurityArchProtocol {
                file_authentication_state: arch_protocol.file_authentication_state,
            },
            original_hook2: SecurityArch2Protocol {
                file_authentication: arch_protocol2.file_authentication,
            },
        };

        // Acquire the lock to the global state and replace it.
        let mut global_state = GLOBAL_HOOK_STATE.lock();
        global_state.replace(state);

        // Install the hooks into the UEFI stack.
        arch_protocol.file_authentication_state = Self::arch_file_authentication_state;
        arch_protocol2.file_authentication = Self::arch2_file_authentication;

        Ok(true)
    }

    /// Uninstalls the global security hook, if installed.
    pub fn uninstall() -> Result<()> {
        // Find the security arch protocol. If we can't find it, we will do nothing.
        let Some(hook_arch) = utils::find_handle(&SECURITY_ARCH_GUID)
            .context("unable to check security arch existence")?
        else {
            return Ok(());
        };

        // Find the security arch2 protocol. If we can't find it, we will do nothing.
        let Some(hook_arch2) = utils::find_handle(&SECURITY_ARCH2_GUID)
            .context("unable to check security arch2 existence")?
        else {
            return Ok(());
        };

        // Open the security arch protocol.
        let mut arch_protocol =
            uefi::boot::open_protocol_exclusive::<SecurityArchProtocol>(hook_arch)
                .context("unable to open security arch protocol")?;

        // Open the security arch2 protocol.
        let mut arch_protocol2 =
            uefi::boot::open_protocol_exclusive::<SecurityArch2Protocol>(hook_arch2)
                .context("unable to open security arch2 protocol")?;

        // Acquire the lock to the global state.
        let mut global_state = GLOBAL_HOOK_STATE.lock();

        // Take the state and replace the original functions.
        let Some(state) = global_state.take() else {
            return Ok(());
        };

        // Reinstall the original functions.
        arch_protocol.file_authentication_state = state.original_hook.file_authentication_state;
        arch_protocol2.file_authentication = state.original_hook2.file_authentication;
        Ok(())
    }
}

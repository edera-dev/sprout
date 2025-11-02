use crate::integrations::shim::{ShimInput, ShimSupport, ShimVerificationOutput};
use crate::utils;
use anyhow::{Context, Result, bail};
use log::warn;
use std::sync::{LazyLock, Mutex};
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
        path: *mut FfiDevicePath,
    ) -> Status,
}

/// EFI_SECURITY_ARCH2 protocol definition.
#[unsafe_protocol(SECURITY_ARCH2_GUID)]
pub struct SecurityArch2Protocol {
    /// Determines the file authentication.
    pub file_authentication: unsafe extern "efiapi" fn(
        this: *const SecurityArch2Protocol,
        path: *mut FfiDevicePath,
        file_buffer: *mut u8,
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
static GLOBAL_HOOK_STATE: LazyLock<Mutex<Option<SecurityHookState>>> =
    LazyLock::new(|| Mutex::new(None));

/// Security hook helper.
pub struct SecurityHook;

impl SecurityHook {
    /// Shared verifier logic for both hook types.
    fn verify(input: ShimInput) -> Status {
        // Verify the input.
        match ShimSupport::verify(input) {
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
        }
    }

    /// File authentication state verifier for the EFI_SECURITY_ARCH protocol.
    /// Takes the `path` and determines the verification.
    unsafe extern "efiapi" fn arch_file_authentication_state(
        _this: *const SecurityArchProtocol,
        _status: u32,
        path: *mut FfiDevicePath,
    ) -> Status {
        // Verify the path is not null.
        if path.is_null() {
            return Status::INVALID_PARAMETER;
        }

        // Construct a shim input from the path.
        let input = ShimInput::SecurityHookPath(path);

        // Verify the input.
        Self::verify(input)
    }

    /// File authentication verifier for the EFI_SECURITY_ARCH2 protocol.
    /// Takes the `path` and a file buffer to determine the verification.
    unsafe extern "efiapi" fn arch2_file_authentication(
        _this: *const SecurityArch2Protocol,
        path: *mut FfiDevicePath,
        file_buffer: *mut u8,
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
        let buffer = unsafe { std::slice::from_raw_parts(file_buffer, file_size) };

        // Construct a shim input from the path.
        let input = ShimInput::SecurityHookBuffer(Some(path), buffer);

        // Verify the input.
        Self::verify(input)
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
        let Ok(mut global_state) = GLOBAL_HOOK_STATE.lock() else {
            bail!("unable to acquire global hook state lock");
        };
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
        let Ok(mut global_state) = GLOBAL_HOOK_STATE.lock() else {
            bail!("unable to acquire global hook state lock");
        };

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

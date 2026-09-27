use crate::shim::{ShimInput, ShimSupport, ShimVerificationOutput};
use anyhow::{Context, Result};
use core::slice;
use log::warn;
use spin::{LazyLock, Mutex};
use uefi::proto::device_path::FfiDevicePath;
use uefi::proto::unsafe_protocol;
use uefi::{Guid, guid};
use uefi_raw::{Boolean, Status};

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
        boot_policy: Boolean,
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

    /// Call the original EFI_SECURITY_ARCH hook with `this`, `status`, and `path`.
    /// This lets the firmware decide on images that the shim can't verify.
    unsafe fn original_file_authentication_state(
        this: *const SecurityArchProtocol,
        status: u32,
        path: *const FfiDevicePath,
    ) -> Status {
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
    }

    /// Call the original EFI_SECURITY_ARCH2 hook with `this`, `path`, `file_buffer`,
    /// `file_size`, and `boot_policy`.
    /// This lets the firmware decide on images that the shim can't verify.
    unsafe fn original_file_authentication(
        this: *const SecurityArch2Protocol,
        path: *const FfiDevicePath,
        file_buffer: *const u8,
        file_size: usize,
        boot_policy: Boolean,
    ) -> Status {
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
    }

    /// File authentication state verifier for the EFI_SECURITY_ARCH protocol.
    /// Takes the `path` and determines the verification.
    /// Anything the shim can't verify is passed to the original hook.
    unsafe extern "efiapi" fn arch_file_authentication_state(
        this: *const SecurityArchProtocol,
        status: u32,
        path: *const FfiDevicePath,
    ) -> Status {
        // Without a path, there is nothing for the shim to read and verify.
        if path.is_null() {
            // SAFETY: The arguments are passed through unchanged from the firmware.
            return unsafe { Self::original_file_authentication_state(this, status, path) };
        }

        // Construct a shim input from the path.
        let input = ShimInput::SecurityHookPath(path);

        // Convert the input to an owned data buffer.
        let input = match input.into_owned_data_buffer() {
            Ok(input) => input,
            // If the data can't be read, such as for a path the shim can't access,
            // let the original hook decide.
            Err(error) => {
                warn!("unable to read data to be authenticated: {}", error);
                // SAFETY: The arguments are passed through unchanged from the firmware.
                return unsafe { Self::original_file_authentication_state(this, status, path) };
            }
        };

        // Verify the input, if it fails, call the original hook.
        if !Self::verify(input) {
            // SAFETY: The arguments are passed through unchanged from the firmware.
            unsafe { Self::original_file_authentication_state(this, status, path) }
        } else {
            Status::SUCCESS
        }
    }

    /// File authentication verifier for the EFI_SECURITY_ARCH2 protocol.
    /// Takes the `path` and a file buffer to determine the verification.
    /// Anything the shim can't verify is passed to the original hook.
    unsafe extern "efiapi" fn arch2_file_authentication(
        this: *const SecurityArch2Protocol,
        path: *const FfiDevicePath,
        file_buffer: *const u8,
        file_size: usize,
        boot_policy: Boolean,
    ) -> Status {
        // The path and file buffer are optional, and the shim can't verify without them.
        // The shim also doesn't support the boot policy.
        if path.is_null() || file_buffer.is_null() || bool::from(boot_policy) {
            // SAFETY: The arguments are passed through unchanged from the firmware.
            return unsafe {
                Self::original_file_authentication(this, path, file_buffer, file_size, boot_policy)
            };
        }

        // Construct a slice out of the file buffer and size.
        let buffer = unsafe { slice::from_raw_parts(file_buffer, file_size) };

        // Construct a shim input from the path.
        let input = ShimInput::SecurityHookBuffer(Some(path), buffer);

        // Verify the input, if it fails, call the original hook.
        if !Self::verify(input) {
            // SAFETY: The arguments are passed through unchanged from the firmware.
            unsafe {
                Self::original_file_authentication(this, path, file_buffer, file_size, boot_policy)
            }
        } else {
            Status::SUCCESS
        }
    }

    /// Install the security hook if needed.
    pub fn install() -> Result<bool> {
        // Find the security arch protocol. If we can't find it, we will return false.
        let Some(hook_arch) = crate::handle::find_handle(&SECURITY_ARCH_GUID)
            .context("unable to check security arch existence")?
        else {
            return Ok(false);
        };

        // Find the security arch2 protocol. If we can't find it, we will return false.
        let Some(hook_arch2) = crate::handle::find_handle(&SECURITY_ARCH2_GUID)
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

        // If the hook is already installed, the protocols contain our hooks.
        // Storing them as the original hooks would cause infinite recursion on
        // verification failure, so keep the existing state instead.
        if global_state.is_some() {
            return Ok(true);
        }
        global_state.replace(state);

        // Install the hooks into the UEFI stack.
        arch_protocol.file_authentication_state = Self::arch_file_authentication_state;
        arch_protocol2.file_authentication = Self::arch2_file_authentication;

        Ok(true)
    }

    /// Uninstalls the global security hook, if installed.
    pub fn uninstall() -> Result<()> {
        // Find the security arch protocol. If we can't find it, we will do nothing.
        let Some(hook_arch) = crate::handle::find_handle(&SECURITY_ARCH_GUID)
            .context("unable to check security arch existence")?
        else {
            return Ok(());
        };

        // Find the security arch2 protocol. If we can't find it, we will do nothing.
        let Some(hook_arch2) = crate::handle::find_handle(&SECURITY_ARCH2_GUID)
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

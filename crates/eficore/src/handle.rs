use anyhow::{Context, Result};
use uefi::boot::{OpenProtocolAttributes, OpenProtocolParams, ScopedProtocol, SearchType};
use uefi::proto::ProtocolPointer;
use uefi::{Guid, Handle};
use uefi_raw::Status;

/// Find a handle that provides the specified `protocol`.
pub fn find_handle(protocol: &Guid) -> Result<Option<Handle>> {
    // Locate the requested protocol handle.
    match uefi::boot::locate_handle_buffer(SearchType::ByProtocol(protocol)) {
        // If a handle is found, the protocol is available.
        Ok(handles) => Ok(if handles.is_empty() {
            None
        } else {
            Some(handles[0])
        }),
        // If an error occurs, check if it is because the protocol is not available.
        // If so, return false. Otherwise, return the error.
        Err(error) => {
            if error.status() == Status::NOT_FOUND {
                Ok(None)
            } else {
                Err(error).context("unable to determine if the protocol is available")
            }
        }
    }
}

/// Open the protocol `P` on `handle` without taking it from the firmware.
/// An exclusive open makes the firmware disconnect every driver that is using the protocol,
/// which for a disk or a partition would remove the filesystem Sprout is running from.
pub fn open_shared<P: ProtocolPointer + ?Sized>(handle: Handle) -> uefi::Result<ScopedProtocol<P>> {
    // SAFETY: The protocols opened this way are only used from this thread, and the firmware
    // keeps them installed for as long as they are open.
    unsafe {
        uefi::boot::open_protocol::<P>(
            OpenProtocolParams {
                handle,
                agent: uefi::boot::image_handle(),
                controller: None,
            },
            OpenProtocolAttributes::GetProtocol,
        )
    }
}

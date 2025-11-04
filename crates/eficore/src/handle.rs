use anyhow::{Context, Result};
use uefi::boot::SearchType;
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

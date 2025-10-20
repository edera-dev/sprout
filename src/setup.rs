use anyhow::{Context, Result};
use std::os::uefi as uefi_std;

/// Initializes the UEFI environment.
///
/// This fetches the system table and current image handle from uefi_std and injects
/// them into the uefi crate.
pub fn init() -> Result<()> {
    // Acquire the system table and image handle from the uefi_std environment.
    let system_table = uefi_std::env::system_table();
    let image_handle = uefi_std::env::image_handle();

    // SAFETY: The UEFI variables above come from the Rust std.
    // These variables are not-null and calling the uefi crates with these values is validated
    // to be corrected by hand.
    unsafe {
        // Set the system table and image handle.
        uefi::table::set_system_table(system_table.as_ptr().cast());
        let handle = uefi::Handle::from_ptr(image_handle.as_ptr().cast())
            .context("unable to resolve image handle")?;
        uefi::boot::set_image_handle(handle);
    }

    // Initialize the uefi logger mechanism and other helpers.
    uefi::helpers::init().context("unable to initialize uefi")?;
    Ok(())
}

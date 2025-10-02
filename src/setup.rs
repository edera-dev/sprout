use std::os::uefi as uefi_std;

pub fn init() {
    let system_table = uefi_std::env::system_table();
    let image_handle = uefi_std::env::image_handle();

    // SAFETY: The uefi variables above come from the Rust std.
    // These variables are nonnull and calling the uefi crates with these values is validated
    // to be corrected by hand.
    unsafe {
        uefi::table::set_system_table(system_table.as_ptr().cast());
        let handle = uefi::Handle::from_ptr(image_handle.as_ptr().cast())
            .expect("unable to resolve image handle");
        uefi::boot::set_image_handle(handle);
    }

    uefi::helpers::init().expect("failed to initialize uefi");
}

#![feature(uefi_std)]
pub mod chainload;
pub mod setup;

const CHAINLOADER_TARGET: &str = "\\EFI\\BOOT\\KERNEL.efi";

fn main() {
    setup::init();
    chainload::chainload(CHAINLOADER_TARGET);
}

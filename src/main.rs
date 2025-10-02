#![feature(uefi_std)]
pub mod config;
pub mod modules;
pub mod setup;

fn main() {
    setup::init();

    let config = config::load();
    modules::execute(config.modules);
}

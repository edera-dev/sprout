#![feature(uefi_std)]
pub mod config;
pub mod modules;
pub mod setup;
pub mod utils;

fn main() {
    setup::init();

    let config = config::load();
    for module in config.modules {
        modules::execute(module);
    }
}

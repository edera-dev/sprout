use crate::config::ModuleConfiguration;

pub mod chainloader;

pub fn execute(module: ModuleConfiguration) {
    if let Some(chainloader) = module.chainloader {
        chainloader::chainloader(chainloader);
    } else {
        panic!("unknown module configuration");
    }
}

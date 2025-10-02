use crate::config::ModuleConfiguration;

pub mod chainloader;

pub fn execute(modules: Vec<ModuleConfiguration>) {
    for module in modules {
        if let Some(chainloader) = module.chainloader {
            chainloader::chainloader(chainloader);
        } else {
            panic!("unknown module configuration");
        }
    }
}

use crate::utils;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
pub struct RootConfiguration {
    #[serde(default)]
    pub modules: Vec<ModuleConfiguration>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct ModuleConfiguration {
    #[serde(default)]
    pub chainloader: Option<ChainloaderConfiguration>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct ChainloaderConfiguration {
    pub path: String,
    #[serde(default)]
    pub options: Vec<String>,
}

pub fn load() -> RootConfiguration {
    let content = utils::read_file_contents("sprout.toml");
    toml::from_slice(&content).expect("unable to parse sprout.toml file")
}

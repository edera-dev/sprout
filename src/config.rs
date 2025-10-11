use crate::actions::ActionDeclaration;
use crate::generators::GeneratorDeclaration;
use crate::utils;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct RootConfiguration {
    #[serde(default = "latest_version")]
    pub version: u32,
    #[serde(default)]
    pub values: BTreeMap<String, String>,
    #[serde(default)]
    pub actions: BTreeMap<String, ActionDeclaration>,
    #[serde(default)]
    pub entries: BTreeMap<String, EntryDeclaration>,
    #[serde(default)]
    pub generators: BTreeMap<String, GeneratorDeclaration>,
    #[serde(default)]
    pub phases: PhasesConfiguration,
}

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct EntryDeclaration {
    pub title: String,
    #[serde(default)]
    pub actions: Vec<String>,
    #[serde(default)]
    pub values: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct PhasesConfiguration {
    #[serde(default)]
    pub startup: Vec<PhaseConfiguration>,
}

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct PhaseConfiguration {
    #[serde(default)]
    pub actions: Vec<String>,
    #[serde(default)]
    pub values: BTreeMap<String, String>,
}

pub fn load() -> RootConfiguration {
    let content = utils::read_file_contents("sprout.toml");
    toml::from_slice(&content).expect("unable to parse sprout.toml file")
}

pub fn latest_version() -> u32 {
    1
}

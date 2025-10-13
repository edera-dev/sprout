use crate::actions::ActionDeclaration;
use crate::drivers::DriverDeclaration;
use crate::extractors::ExtractorDeclaration;
use crate::generators::GeneratorDeclaration;
use crate::utils;
use anyhow::Context;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::ops::Deref;
use uefi::proto::device_path::LoadedImageDevicePath;

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct RootConfiguration {
    #[serde(default = "latest_version")]
    pub version: u32,
    #[serde(default)]
    pub values: BTreeMap<String, String>,
    #[serde(default)]
    pub drivers: BTreeMap<String, DriverDeclaration>,
    #[serde(default)]
    pub extractors: BTreeMap<String, ExtractorDeclaration>,
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

pub fn load() -> Result<RootConfiguration> {
    let current_image_device_path_protocol =
        uefi::boot::open_protocol_exclusive::<LoadedImageDevicePath>(uefi::boot::image_handle())
            .context("failed to get loaded image device path")?;
    let path = current_image_device_path_protocol.deref().to_boxed();

    let content = utils::read_file_contents(&path, "sprout.toml")
        .context("unable to read sprout.toml file")?;
    let config: RootConfiguration =
        toml::from_slice(&content).context("unable to parse sprout.toml file")?;
    Ok(config)
}

pub fn latest_version() -> u32 {
    1
}

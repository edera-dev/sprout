use crate::actions::ActionDeclaration;
use crate::drivers::DriverDeclaration;
use crate::entries::EntryDeclaration;
use crate::extractors::ExtractorDeclaration;
use crate::generators::GeneratorDeclaration;
use crate::phases::PhasesConfiguration;
use crate::utils;
use anyhow::Result;
use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::ops::Deref;
use toml::Value;
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

pub fn latest_version() -> u32 {
    1
}

fn load_raw_config() -> Result<Vec<u8>> {
    let current_image_device_path_protocol =
        uefi::boot::open_protocol_exclusive::<LoadedImageDevicePath>(uefi::boot::image_handle())
            .context("unable to get loaded image device path")?;
    let path = current_image_device_path_protocol.deref().to_boxed();

    let content = utils::read_file_contents(&path, "sprout.toml")
        .context("unable to read sprout.toml file")?;
    Ok(content)
}

pub fn load() -> Result<RootConfiguration> {
    let content = load_raw_config()?;
    let value: Value = toml::from_slice(&content).context("unable to parse sprout.toml file")?;

    let version = value
        .get("version")
        .cloned()
        .unwrap_or_else(|| Value::Integer(latest_version() as i64));

    let version: u32 = version
        .try_into()
        .context("unable to get configuration version")?;

    if version != latest_version() {
        bail!("unsupported configuration version: {}", version);
    }

    let config: RootConfiguration = value
        .try_into()
        .context("unable to parse sprout.toml file")?;
    Ok(config)
}

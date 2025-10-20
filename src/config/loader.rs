use std::ops::Deref;
use anyhow::{bail, Context, Result};
use toml::Value;
use uefi::proto::device_path::LoadedImageDevicePath;
use crate::config::{latest_version, RootConfiguration};
use crate::utils;

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

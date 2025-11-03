use crate::options::SproutOptions;
use crate::platform::tpm::PlatformTpm;
use crate::utils;
use alloc::vec::Vec;
use anyhow::{Context, Result, bail};
use core::ops::Deref;
use edera_sprout_config::{RootConfiguration, latest_version};
use log::info;
use toml::Value;
use uefi::proto::device_path::LoadedImageDevicePath;

/// Loads the raw configuration from the sprout config file as data.
fn load_raw_config(options: &SproutOptions) -> Result<Vec<u8>> {
    // Open the LoadedImageDevicePath protocol to get the path to the current image.
    let current_image_device_path_protocol =
        uefi::boot::open_protocol_exclusive::<LoadedImageDevicePath>(uefi::boot::image_handle())
            .context("unable to get loaded image device path")?;
    // Acquire the device path as a boxed device path.
    let path = current_image_device_path_protocol.deref().to_boxed();

    info!("configuration file: {}", options.config);

    // Read the contents of the sprout config file.
    let content = utils::read_file_contents(Some(&path), &options.config)
        .context("unable to read sprout config file")?;

    // Measure the sprout.toml into the TPM, if needed and possible.
    PlatformTpm::log_event(
        PlatformTpm::PCR_BOOT_LOADER_CONFIG,
        &content,
        "sprout: configuration file",
    )
    .context("unable to measure the sprout.toml file into the TPM")?;

    // Return the contents of the sprout config file.
    Ok(content)
}

/// Loads the [RootConfiguration] for Sprout.
pub fn load(options: &SproutOptions) -> Result<RootConfiguration> {
    // Load the raw configuration from the sprout config file.
    let content = load_raw_config(options)?;
    // Parse the raw configuration into a toml::Value which can represent any TOML file.
    let value: Value = toml::from_slice(&content).context("unable to parse sprout config file")?;

    // Check the version of the configuration without parsing the full configuration.
    let version = value
        .get("version")
        .cloned()
        .unwrap_or_else(|| Value::Integer(latest_version() as i64));

    // Parse the version into an u32.
    let version: u32 = version
        .try_into()
        .context("unable to get configuration version")?;

    // Check if the version is supported.
    if version != latest_version() {
        bail!("unsupported configuration version: {}", version);
    }

    // If the version is supported, parse the full configuration.
    let config: RootConfiguration = value
        .try_into()
        .context("unable to parse sprout.toml file")?;

    // Return the parsed configuration.
    Ok(config)
}

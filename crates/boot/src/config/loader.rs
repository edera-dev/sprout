use crate::options::SproutOptions;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use anyhow::{Context, Result, bail};
use core::fmt::Display;
use core::ops::Deref;
use edera_sprout_config::{RootConfiguration, latest_version};
use eficore::platform::tpm::PlatformTpm;
use log::{info, warn};
use serde_ignored::Path;
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
    let content = eficore::path::read_file_contents(Some(&path), &options.config)
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

/// Format the configuration key `path` with dots, like "actions.linux.chainload.path".
fn key_path(path: &Path) -> String {
    // Joins the `parent` path and the `key` with a dot, unless the parent is the root.
    fn join(parent: &Path, key: impl Display) -> String {
        match parent {
            Path::Root => key.to_string(),
            parent => format!("{}.{}", key_path(parent), key),
        }
    }

    match path {
        Path::Root => String::new(),
        Path::Seq { parent, index } => join(parent, index),
        Path::Map { parent, key } => join(parent, key),
        // Optional values and wrapper types don't add a key of their own.
        Path::Some { parent }
        | Path::NewtypeStruct { parent }
        | Path::NewtypeVariant { parent } => key_path(parent),
    }
}

/// Warn about each declaration in `config` that configures more than one kind.
/// For example, an action with both chainload and print configured only runs chainload.
fn warn_multiple_kinds(config: &RootConfiguration) {
    for (name, action) in &config.actions {
        let kinds = [
            action.chainload.is_some(),
            action.print.is_some(),
            action.edera.is_some(),
        ];
        if kinds.iter().filter(|kind| **kind).count() > 1 {
            warn!(
                "action {} configures more than one action, only one is used",
                name
            );
        }
    }

    for (name, generator) in &config.generators {
        let kinds = [
            generator.matrix.is_some(),
            generator.bls.is_some(),
            generator.list.is_some(),
        ];
        if kinds.iter().filter(|kind| **kind).count() > 1 {
            warn!(
                "generator {} configures more than one generator, only one is used",
                name
            );
        }
    }
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
    // Keys that aren't part of the configuration, such as misspelled keys, are ignored.
    // Warn about each one, as the configuration may not do what was intended.
    let config: RootConfiguration = serde_ignored::deserialize(value, |path| {
        warn!("ignoring unknown configuration key: {}", key_path(&path));
    })
    .context("unable to parse sprout.toml file")?;

    // Warn about declarations that configure more than one kind, as only one is used.
    warn_multiple_kinds(&config);

    // Return the parsed configuration.
    Ok(config)
}

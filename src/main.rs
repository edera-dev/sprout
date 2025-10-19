#![feature(uefi_std)]

use crate::context::{RootContext, SproutContext};
use crate::phases::phase;
use anyhow::{Context, Result, bail};
use log::info;
use std::collections::BTreeMap;
use std::ops::Deref;
use uefi::proto::device_path::LoadedImageDevicePath;
use uefi::proto::device_path::text::{AllowShortcuts, DisplayOnly};

pub mod actions;
pub mod config;
pub mod context;
pub mod drivers;
pub mod entries;
pub mod extractors;
pub mod generators;
pub mod phases;
pub mod setup;
pub mod utils;

fn main() -> Result<()> {
    setup::init()?;

    let config = config::load()?;

    if config.version != config::latest_version() {
        bail!("unsupported configuration version: {}", config.version);
    }

    let mut root = {
        let current_image_device_path_protocol = uefi::boot::open_protocol_exclusive::<
            LoadedImageDevicePath,
        >(uefi::boot::image_handle())
        .context("unable to get loaded image device path")?;
        let loaded_image_path = current_image_device_path_protocol.deref().to_boxed();
        info!(
            "loaded image path: {}",
            loaded_image_path.to_string(DisplayOnly(false), AllowShortcuts(false))?
        );
        RootContext::new(loaded_image_path)
    };

    root.actions_mut().extend(config.actions.clone());

    let mut context = SproutContext::new(root);
    context.insert(&config.values);
    let context = context.freeze();

    phase(context.clone(), &config.phases.early).context("unable to execute early phase")?;

    drivers::load(context.clone(), &config.drivers).context("unable to load drivers")?;

    let mut extracted = BTreeMap::new();
    for (name, extractor) in &config.extractors {
        let value = extractors::extract(context.clone(), extractor)
            .context(format!("unable to extract value {}", name))?;
        info!("extracted value {}: {}", name, value);
        extracted.insert(name.clone(), value);
    }
    let mut context = context.fork();
    context.insert(&extracted);
    let context = context.freeze();

    phase(context.clone(), &config.phases.startup).context("unable to execute startup phase")?;

    let mut all_entries = Vec::new();

    for (_name, entry) in config.entries {
        all_entries.push((context.clone(), entry));
    }

    for (_name, generator) in config.generators {
        let context = context.fork().freeze();

        for entry in generators::generate(context.clone(), &generator)? {
            all_entries.push(entry);
        }
    }

    let mut final_entries = Vec::new();
    for (context, entry) in all_entries {
        let mut context = context.fork();
        context.insert(&entry.values);
        let context = context.finalize().freeze();

        final_entries.push((context, entry));
    }

    info!("entries:");
    for (index, (context, entry)) in final_entries.iter().enumerate() {
        let title = context.stamp(&entry.title);
        info!("  entry {}: {}", index + 1, title);
    }

    phase(context.clone(), &config.phases.late).context("unable to execute late phase")?;

    let Some((context, entry)) = final_entries.first() else {
        bail!("no entries found");
    };

    for action in &entry.actions {
        let action = context.stamp(action);
        actions::execute(context.clone(), &action)
            .context(format!("unable to execute action '{}'", action))?;
    }
    Ok(())
}

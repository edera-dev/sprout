#![doc = include_str!("../README.md")]
#![feature(uefi_std)]

use crate::config::RootConfiguration;
use crate::context::{RootContext, SproutContext};
use crate::entries::BootableEntry;
use crate::options::SproutOptions;
use crate::options::parser::OptionsRepresentable;
use crate::phases::phase;
use anyhow::{Context, Result, bail};
use log::{error, info};
use std::collections::BTreeMap;
use std::ops::Deref;
use std::time::Duration;
use uefi::proto::device_path::LoadedImageDevicePath;

/// actions: Code that can be configured and executed by Sprout.
pub mod actions;

/// autoconfigure: Autoconfigure Sprout based on the detected environment.
pub mod autoconfigure;

/// config: Sprout configuration mechanism.
pub mod config;

/// context: Stored values that can be cheaply forked and cloned.
pub mod context;

/// drivers: EFI drivers to load and provide extra functionality.
pub mod drivers;

/// entries: Boot menu entries that have a title and can execute actions.
pub mod entries;

/// extractors: Runtime code that can extract values into the Sprout context.
pub mod extractors;

/// generators: Runtime code that can generate entries with specific values.
pub mod generators;

/// menu: Display a boot menu to select an entry to boot.
pub mod menu;

/// phases: Hooks into specific parts of the boot process.
pub mod phases;

/// setup: Code that initializes the UEFI environment for Sprout.
pub mod setup;

/// options: Parse the options of the Sprout executable.
pub mod options;

/// utils: Utility functions that are used by other parts of Sprout.
pub mod utils;

/// Run Sprout, returning an error if one occurs.
fn run() -> Result<()> {
    // Parse the options to the sprout executable.
    let options = SproutOptions::parse().context("unable to parse options")?;

    // If --autoconfigure is specified, we use a stub configuration.
    let mut config = if options.autoconfigure {
        info!("autoconfiguration enabled, configuration file will be ignored");
        RootConfiguration::default()
    } else {
        // Load the configuration of sprout.
        // At this point, the configuration has been validated and the specified
        // version is checked to ensure compatibility.
        config::loader::load(&options)?
    };

    // Load the root context.
    // This is done in a block to ensure the release of the LoadedImageDevicePath protocol.
    let mut root = {
        let current_image_device_path_protocol = uefi::boot::open_protocol_exclusive::<
            LoadedImageDevicePath,
        >(uefi::boot::image_handle())
        .context("unable to get loaded image device path")?;
        let loaded_image_path = current_image_device_path_protocol.deref().to_boxed();
        RootContext::new(loaded_image_path, options)
    };

    // Insert the configuration actions into the root context.
    root.actions_mut().extend(config.actions.clone());

    // Create a new sprout context with the root context.
    let mut context = SproutContext::new(root);

    // Insert the configuration values into the sprout context.
    context.insert(&config.values);

    // Freeze the sprout context so it can be shared and cheaply cloned.
    let context = context.freeze();

    // Execute the early phase.
    phase(context.clone(), &config.phases.early).context("unable to execute early phase")?;

    // Load all configured drivers.
    drivers::load(context.clone(), &config.drivers).context("unable to load drivers")?;

    // If --autoconfigure is specified or the loaded configuration has autoconfigure enabled,
    // trigger the autoconfiguration mechanism.
    if context.root().options().autoconfigure || config.options.autoconfigure {
        autoconfigure::autoconfigure(&mut config).context("unable to autoconfigure")?;
    }

    // Unload the context so that it can be modified.
    let Some(mut context) = context.unload() else {
        bail!("context safety violation while trying to unload context");
    };

    // Perform root context modification in a block to release the modification when complete.
    {
        // Modify the root context to include the autoconfigured actions.
        let Some(root) = context.root_mut() else {
            bail!("context safety violation while trying to modify root context");
        };

        // Extend the root context with the autoconfigured actions.
        root.actions_mut().extend(config.actions);

        // Insert any modified root values.
        context.insert(&config.values);
    }

    // Refreeze the context to ensure that further operations can share the context.
    let context = context.freeze();

    // Run all the extractors declared in the configuration.
    let mut extracted = BTreeMap::new();
    for (name, extractor) in &config.extractors {
        let value = extractors::extract(context.clone(), extractor)
            .context(format!("unable to extract value {}", name))?;
        info!("extracted value {}: {}", name, value);
        extracted.insert(name.clone(), value);
    }
    let mut context = context.fork();
    // Insert the extracted values into the sprout context.
    context.insert(&extracted);
    let context = context.freeze();

    // Execute the startup phase.
    phase(context.clone(), &config.phases.startup).context("unable to execute startup phase")?;

    let mut entries = Vec::new();

    // Insert all the static entries from the configuration into the entry list.
    for (name, entry) in config.entries {
        // Associate the main context with the static entry.
        entries.push(BootableEntry::new(
            name,
            entry.title.clone(),
            context.clone(),
            entry,
        ));
    }

    // Run all the generators declared in the configuration.
    for (name, generator) in config.generators {
        let context = context.fork().freeze();

        // We will prefix all entries with [name]-.
        let prefix = format!("{}-", name);

        // Add all the entries generated by the generator to the entry list.
        // The generator specifies the context associated with the entry.
        for mut entry in generators::generate(context.clone(), &generator)? {
            entry.prepend_name_prefix(&prefix);
            entries.push(entry);
        }
    }

    for entry in &mut entries {
        let mut context = entry.context().fork();
        // Insert the values from the entry configuration into the
        // sprout context to use with the entry itself.
        context.insert(&entry.declaration().values);
        let context = context
            .finalize()
            .context("unable to finalize context")?
            .freeze();
        // Provide the new context to the bootable entry.
        entry.swap_context(context);
        // Restamp the title with any values.
        entry.restamp_title();

        // Mark this entry as the default entry if it is declared as such.
        if let Some(ref default_entry) = config.options.default_entry {
            // If the entry matches the default entry, mark it as the default entry.
            if entry.is_match(default_entry) {
                entry.mark_default();
            }
        }
    }

    // If no entries were the default, pick the first entry as the default entry.
    if entries.iter().all(|entry| !entry.is_default())
        && let Some(entry) = entries.first_mut()
    {
        entry.mark_default();
    }

    // Execute the late phase.
    phase(context.clone(), &config.phases.late).context("unable to execute late phase")?;

    // If --boot is specified, boot that entry immediately.
    let force_boot_entry = context.root().options().boot.as_ref();
    // If --force-menu is specified, show the boot menu regardless of the value of --boot.
    let force_boot_menu = context.root().options().force_menu;

    // Determine the menu timeout in seconds based on the options or configuration.
    // We prefer the options over the configuration to allow for overriding.
    let menu_timeout = context
        .root()
        .options()
        .menu_timeout
        .unwrap_or(config.options.menu_timeout);
    let menu_timeout = Duration::from_secs(menu_timeout);

    // Use the forced boot entry if possible, otherwise pick the first entry using a boot menu.
    let entry = if !force_boot_menu && let Some(ref force_boot_entry) = force_boot_entry {
        BootableEntry::find(force_boot_entry, entries.iter())
            .context(format!("unable to find entry: {force_boot_entry}"))?
    } else {
        // Delegate to the menu to select an entry to boot.
        menu::select(menu_timeout, &entries).context("unable to select entry via boot menu")?
    };

    // Execute all the actions for the selected entry.
    for action in &entry.declaration().actions {
        let action = entry.context().stamp(action);
        actions::execute(entry.context().clone(), &action)
            .context(format!("unable to execute action '{}'", action))?;
    }

    Ok(())
}

/// The main entrypoint of sprout.
/// It is possible this function will not return if actions that are executed
/// exit boot services or do not return control to sprout.
fn main() -> Result<()> {
    // Initialize the basic UEFI environment.
    setup::init()?;

    // Run Sprout, then handle the error.
    let result = run();
    if let Err(ref error) = result {
        // Print an error trace.
        error!("sprout encountered an error");
        for (index, stack) in error.chain().enumerate() {
            error!("[{}]: {}", index, stack);
        }
        // Sleep for 10 seconds to allow the user to read the error.
        uefi::boot::stall(Duration::from_secs(10));
    }

    // Sprout doesn't necessarily guarantee anything was booted.
    // If we reach here, we will exit back to whoever called us.
    Ok(())
}

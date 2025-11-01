#![doc = include_str!("../README.md")]
#![feature(uefi_std)]

/// The delay to wait for when an error occurs in Sprout.
const DELAY_ON_ERROR: Duration = Duration::from_secs(10);

use crate::config::RootConfiguration;
use crate::context::{RootContext, SproutContext};
use crate::entries::BootableEntry;
use crate::integrations::bootloader_interface::{BootloaderInterface, BootloaderInterfaceTimeout};
use crate::options::SproutOptions;
use crate::options::parser::OptionsRepresentable;
use crate::phases::phase;
use crate::platform::timer::PlatformTimer;
use crate::platform::tpm::PlatformTpm;
use crate::secure::SecureBoot;
use crate::utils::PartitionGuidForm;
use anyhow::{Context, Result, bail};
use log::{error, info, warn};
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

/// platform: Integration or support code for specific hardware platforms.
pub mod platform;

/// menu: Display a boot menu to select an entry to boot.
pub mod menu;

/// integrations: Code that interacts with other systems.
pub mod integrations;

/// phases: Hooks into specific parts of the boot process.
pub mod phases;

/// sbat: Secure Boot Attestation section.
pub mod sbat;

/// secure: Secure Boot support.
pub mod secure;

/// setup: Code that initializes the UEFI environment for Sprout.
pub mod setup;

/// options: Parse the options of the Sprout executable.
pub mod options;

/// utils: Utility functions that are used by other parts of Sprout.
pub mod utils;

/// Run Sprout, returning an error if one occurs.
fn run() -> Result<()> {
    // For safety reasons, we will note that Secure Boot is in beta on Sprout.
    if SecureBoot::enabled().context("unable to determine Secure Boot status")? {
        warn!("Secure Boot is enabled. Sprout Secure Boot is in beta.");
    }

    // Start the platform timer.
    let timer = PlatformTimer::start();

    // Mark the initialization of Sprout in the bootloader interface.
    BootloaderInterface::mark_init(&timer)
        .context("unable to mark initialization in bootloader interface")?;

    // Tell the bootloader interface what firmware we are running on.
    BootloaderInterface::set_firmware_info()
        .context("unable to set firmware info in bootloader interface")?;

    // Tell the bootloader interface what loader is being used.
    BootloaderInterface::set_loader_info()
        .context("unable to set loader info in bootloader interface")?;

    // Acquire the number of active PCR banks on the TPM.
    // If no TPM is available, this will return zero.
    let active_pcr_banks = PlatformTpm::active_pcr_banks()?;
    // Tell the bootloader interface what the number of active PCR banks is.
    BootloaderInterface::set_tpm2_active_pcr_banks(active_pcr_banks)
        .context("unable to set tpm2 active PCR banks in bootloader interface")?;

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

    // Grab the sprout.efi loaded image path.
    // This is done in a block to ensure the release of the LoadedImageDevicePath protocol.
    let loaded_image_path = {
        let current_image_device_path_protocol = uefi::boot::open_protocol_exclusive::<
            LoadedImageDevicePath,
        >(uefi::boot::image_handle())
        .context("unable to get loaded image device path")?;
        current_image_device_path_protocol.deref().to_boxed()
    };

    // Grab the partition GUID of the ESP that sprout was loaded from.
    let loaded_image_partition_guid =
        utils::partition_guid(&loaded_image_path, PartitionGuidForm::Partition)
            .context("unable to retrieve loaded image partition guid")?;

    // Set the partition GUID of the ESP that sprout was loaded from in the bootloader interface.
    if let Some(loaded_image_partition_guid) = loaded_image_partition_guid {
        // Tell the system about the partition GUID.
        BootloaderInterface::set_partition_guid(&loaded_image_partition_guid)
            .context("unable to set partition guid in bootloader interface")?;
    }

    // Tell the bootloader interface what the loaded image path is.
    BootloaderInterface::set_loader_path(&loaded_image_path)
        .context("unable to set loader path in bootloader interface")?;

    // Create the root context.
    let mut root = RootContext::new(loaded_image_path, timer, options);

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

        // We will prefix all entries with [name]-, provided the name is not pinned.
        let prefix = format!("{}-", name);

        // Add all the entries generated by the generator to the entry list.
        // The generator specifies the context associated with the entry.
        for mut entry in generators::generate(context.clone(), &generator)? {
            // If the entry name is not pinned, prepend the name prefix.
            if !entry.is_pin_name() {
                entry.prepend_name_prefix(&prefix);
            }

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

    // Tell the bootloader interface what entries are available.
    BootloaderInterface::set_entries(entries.iter().map(|entry| entry.name()))
        .context("unable to set entries in bootloader interface")?;

    // Execute the late phase.
    phase(context.clone(), &config.phases.late).context("unable to execute late phase")?;

    // Acquire the timeout setting from the bootloader interface.
    let bootloader_interface_timeout =
        BootloaderInterface::get_timeout().context("unable to get bootloader interface timeout")?;

    // Acquire the default entry from the bootloader interface.
    let bootloader_interface_default_entry = BootloaderInterface::get_default_entry()
        .context("unable to get bootloader interface default entry")?;

    // Acquire the oneshot entry from the bootloader interface.
    let bootloader_interface_oneshot_entry = BootloaderInterface::get_oneshot_entry()
        .context("unable to get bootloader interface oneshot entry")?;

    // If --boot is specified, boot that entry immediately.
    let mut force_boot_entry = context.root().options().boot.clone();
    // If --force-menu is specified, show the boot menu regardless of the value of --boot.
    let mut force_boot_menu = context.root().options().force_menu;

    // Determine the menu timeout in seconds based on the options or configuration.
    // We prefer the options over the configuration to allow for overriding.
    let mut menu_timeout = context
        .root()
        .options()
        .menu_timeout
        .unwrap_or(config.options.menu_timeout);

    // Apply bootloader interface timeout settings.
    match bootloader_interface_timeout {
        BootloaderInterfaceTimeout::MenuForce => {
            // Force the boot menu.
            force_boot_menu = true;
        }

        BootloaderInterfaceTimeout::MenuHidden | BootloaderInterfaceTimeout::MenuDisabled => {
            // Hide the boot menu by setting the timeout to zero.
            menu_timeout = 0;
        }

        BootloaderInterfaceTimeout::Timeout(timeout) => {
            // Configure the timeout to the specified value.
            menu_timeout = timeout;
        }

        BootloaderInterfaceTimeout::Unspecified => {
            // Do nothing.
        }
    }

    // Apply bootloader interface default entry settings.
    if let Some(ref bootloader_interface_default_entry) = bootloader_interface_default_entry {
        // Iterate over all the entries and mark the default entry as the one specified.
        for entry in &mut entries {
            // Mark the entry as the default entry if it matches the specified entry.
            // If the entry does not match the specified entry, unmark it as the default entry.
            if entry.is_match(bootloader_interface_default_entry) {
                entry.mark_default();
            } else {
                entry.unmark_default();
            }
        }
    }

    // Apply bootloader interface oneshot entry settings.
    // If set, we will force booting the oneshot entry.
    if let Some(ref bootloader_interface_oneshot_entry) = bootloader_interface_oneshot_entry {
        force_boot_entry = Some(bootloader_interface_oneshot_entry.clone());
    }

    // If no entries were the default, pick the first entry as the default entry.
    if entries.iter().all(|entry| !entry.is_default())
        && let Some(entry) = entries.first_mut()
    {
        entry.mark_default();
    }

    // Convert the menu timeout to a duration.
    let menu_timeout = Duration::from_secs(menu_timeout);

    // Use the forced boot entry if possible, otherwise pick the first entry using a boot menu.
    let entry = if !force_boot_menu && let Some(ref force_boot_entry) = force_boot_entry {
        BootableEntry::find(force_boot_entry, entries.iter())
            .context(format!("unable to find entry: {force_boot_entry}"))?
    } else {
        // Delegate to the menu to select an entry to boot.
        menu::select(&timer, menu_timeout, &entries)
            .context("unable to select entry via boot menu")?
    };

    // Tell the bootloader interface what the selected entry is.
    BootloaderInterface::set_selected_entry(entry.name().to_string())
        .context("unable to set selected entry in bootloader interface")?;

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
        // Sleep to allow the user to read the error.
        uefi::boot::stall(DELAY_ON_ERROR);
    }

    // Sprout doesn't necessarily guarantee anything was booted.
    // If we reach here, we will exit back to whoever called us.
    Ok(())
}

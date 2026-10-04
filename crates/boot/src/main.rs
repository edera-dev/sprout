#![doc = include_str!("../README.md")]
#![no_std]
#![no_main]
extern crate alloc;

use crate::{
    context::{RootContext, SproutContext},
    entries::BootableEntry,
    options::SproutOptions,
    phases::phase,
};
use alloc::{
    collections::{BTreeMap, BTreeSet},
    format,
    string::{String, ToString},
    vec::Vec,
};
use anyhow::{Context, Result, bail};
use core::{ops::Deref, time::Duration};
use edera_sprout_bls::{LoaderConf, LoaderTimeout, compare_versions};
use edera_sprout_config::{DEFAULT_MENU_TIMEOUT_SECONDS, RootConfiguration};
use eficore::{
    bootloader_interface::{BootloaderInterface, BootloaderInterfaceTimeout},
    partition::PartitionGuidForm,
    platform::{timer::PlatformTimer, tpm::PlatformTpm},
    secure::SecureBoot,
    setup,
};
use log::{error, info, warn};
use uefi::{entry, proto::device_path::LoadedImageDevicePath};
use uefi_raw::Status;

/// actions: Code that can be configured and executed by Sprout.
pub mod actions;

/// autoconfigure: Autoconfigure Sprout based on the detected environment.
pub mod autoconfigure;

/// boot_counter: Consume the boot counter tries of BLS entries.
pub mod boot_counter;

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

/// options: Parse the options of the Sprout executable.
pub mod options;

/// phases: Hooks into specific parts of the boot process.
pub mod phases;

/// sbat: Secure Boot Attestation section.
pub mod sbat;

/// The delay to wait for when an error occurs in Sprout.
const DELAY_ON_ERROR: Duration = Duration::from_secs(10);

/// Unwraps the `result` of a bootloader interface operation, logging the error if one occurs.
/// The bootloader interface only exchanges information with the operating system, so a
/// firmware variable that can't be read or written should not prevent booting.
/// If an error occurs, the default value is provided instead.
fn advisory<T: Default>(result: Result<T>) -> T {
    result.unwrap_or_else(|error| {
        warn!("{:#}", error);
        T::default()
    })
}

/// Loads the loader.conf of the partition Sprout was loaded from.
/// A missing loader.conf has no settings.
fn load_loader_conf(context: &SproutContext) -> Result<LoaderConf> {
    let Ok(content) = eficore::path::read_file_contents(
        Some(context.root().loaded_image_path()?),
        "\\loader\\loader.conf",
    ) else {
        return Ok(LoaderConf::default());
    };

    // Measure the loader.conf into the TPM, as it changes how Sprout boots.
    // A failure to measure is only a warning, as it was not Sprout's own configuration.
    if let Err(error) = PlatformTpm::log_event(
        PlatformTpm::PCR_BOOT_LOADER_CONFIG,
        &content,
        "sprout: loader.conf",
    ) {
        warn!(
            "unable to measure the loader.conf file into the TPM: {:#}",
            error
        );
    }

    // Read what is valid, as a stray byte should not discard the whole file.
    let content = String::from_utf8_lossy(&content);
    Ok(LoaderConf::parse(&content))
}

/// Run Sprout, returning an error if one occurs.
fn run() -> Result<()> {
    // For safety reasons, we will note that Secure Boot is in beta on Sprout.
    if SecureBoot::enabled().context("unable to determine Secure Boot status")? {
        warn!("Sprout Secure Boot is in beta. Some functionality may not work as expected.");
    }

    // Start the platform timer.
    let timer = PlatformTimer::start();

    // Mark the initialization of Sprout in the bootloader interface.
    advisory(
        BootloaderInterface::mark_init(&timer)
            .context("unable to mark initialization in bootloader interface"),
    );

    // Tell the bootloader interface what firmware we are running on.
    advisory(
        BootloaderInterface::set_firmware_info()
            .context("unable to set firmware info in bootloader interface"),
    );

    // Tell the bootloader interface what loader is being used.
    advisory(
        BootloaderInterface::set_loader_info()
            .context("unable to set loader info in bootloader interface"),
    );

    // Acquire the number of active PCR banks on the TPM.
    // If no TPM is available, this will return zero.
    // This is only reported to the bootloader interface, so a TPM that fails to report its
    // PCR banks is treated as having none, rather than preventing boot.
    let active_pcr_banks = PlatformTpm::active_pcr_banks().unwrap_or_else(|error| {
        warn!("unable to determine the active TPM PCR banks: {}", error);
        0
    });
    // Tell the bootloader interface what the number of active PCR banks is.
    advisory(
        BootloaderInterface::set_tpm2_active_pcr_banks(active_pcr_banks)
            .context("unable to set tpm2 active PCR banks in bootloader interface"),
    );

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
    // This is only reported to the bootloader interface.
    let loaded_image_partition_guid = advisory(
        eficore::partition::partition_guid(&loaded_image_path, PartitionGuidForm::Partition)
            .context("unable to retrieve loaded image partition guid"),
    );

    // Set the partition GUID of the ESP that sprout was loaded from in the bootloader interface.
    if let Some(loaded_image_partition_guid) = loaded_image_partition_guid {
        // Tell the system about the partition GUID.
        advisory(
            BootloaderInterface::set_partition_guid(&loaded_image_partition_guid)
                .context("unable to set partition guid in bootloader interface"),
        );
    }

    // Tell the bootloader interface what the loaded image path is.
    advisory(
        BootloaderInterface::set_loader_path(&loaded_image_path)
            .context("unable to set loader path in bootloader interface"),
    );

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

        // Add all the entries generated by the generator to the entry list.
        // The generator specifies the context associated with the entry.
        entries.extend(generators::generate(&name, context, &generator)?);
    }

    // Entry names are used to select entries, so duplicates make the selection ambiguous.
    // This can happen when two generators pin names, like two BLS generators reading the
    // same BLS directory.
    let mut seen_names = BTreeSet::new();
    for entry in &entries {
        if !seen_names.insert(entry.name()) {
            warn!(
                "more than one entry is named {}, selecting it by name is ambiguous",
                entry.name()
            );
        }
    }

    // Entries whose context can't be finalized are skipped, as one broken entry
    // should not prevent booting any of the other entries.
    entries.retain_mut(|entry| {
        let mut context = entry.context().fork();
        // Insert the values from the entry configuration into the
        // sprout context to use with the entry itself.
        context.insert(&entry.declaration().values);
        let context = match context.finalize() {
            Ok(context) => context.freeze(),
            Err(error) => {
                warn!(
                    "skipping entry {}, unable to finalize context: {:#}",
                    entry.name(),
                    error
                );
                return false;
            }
        };
        // Provide the new context to the bootable entry.
        entry.swap_context(context);
        // Restamp the title and sort key with any values.
        entry.restamp_title();
        entry.restamp_sort_key();

        true
    });

    // Sort the entries by their sort key, finalizing the order to show entries. This happens
    // in reverse order so that entries that would come last show up first in the menu.
    entries.sort_by(|a, b| compare_versions(a.sort_key(), b.sort_key()).reverse());

    // Tell the bootloader interface what entries are available.
    advisory(
        BootloaderInterface::set_entries(entries.iter().map(|entry| entry.id()))
            .context("unable to set entries in bootloader interface"),
    );

    // Load the loader.conf of the partition Sprout was loaded from.
    let loader_conf = load_loader_conf(&context).context("unable to load loader.conf")?;
    for warning in &loader_conf.warnings {
        warn!("{}", warning);
    }

    // Acquire the timeouts from the bootloader interface.
    let bootloader_interface_timeouts = advisory(
        BootloaderInterface::get_timeouts().context("unable to get bootloader interface timeouts"),
    );

    // Acquire the default entry from the bootloader interface.
    let bootloader_interface_default_entry = advisory(
        BootloaderInterface::get_default_entry()
            .context("unable to get bootloader interface default entry"),
    );

    // Acquire the preferred entry from the bootloader interface.
    let bootloader_interface_preferred_entry = advisory(
        BootloaderInterface::get_preferred_entry()
            .context("unable to get bootloader interface preferred entry"),
    );

    // Acquire the entry that was saved by the previous boot.
    let last_booted_entry = advisory(
        BootloaderInterface::get_last_booted_entry()
            .context("unable to get last booted entry from bootloader interface"),
    );

    // Acquire the oneshot entry from the bootloader interface.
    let bootloader_interface_oneshot_entry = advisory(
        BootloaderInterface::get_oneshot_entry()
            .context("unable to get bootloader interface oneshot entry"),
    );

    // If --boot is specified, boot that entry immediately.
    let force_boot_entry = context.root().options().boot.clone();
    // If --force-menu is specified, show the boot menu regardless of the value of --boot.
    let mut force_boot_menu = context.root().options().force_menu;

    // Pick the menu timeout from the first source that specifies one, in this order:
    // the one-shot timeout, --menu-timeout, the configuration, LoaderConfigTimeout, loader.conf.
    let seconds = |seconds: Option<u64>| {
        seconds.map_or(
            BootloaderInterfaceTimeout::Unspecified,
            BootloaderInterfaceTimeout::Timeout,
        )
    };
    let loader_conf_timeout = match loader_conf.timeout {
        None => BootloaderInterfaceTimeout::Unspecified,
        Some(LoaderTimeout::Seconds(0)) | Some(LoaderTimeout::Hidden) => {
            BootloaderInterfaceTimeout::MenuHidden
        }
        Some(LoaderTimeout::Seconds(timeout)) => BootloaderInterfaceTimeout::Timeout(timeout),
        Some(LoaderTimeout::Disabled) => BootloaderInterfaceTimeout::MenuDisabled,
        Some(LoaderTimeout::Force) => BootloaderInterfaceTimeout::MenuForce,
    };
    let timeout = [
        bootloader_interface_timeouts.oneshot,
        seconds(context.root().options().menu_timeout),
        seconds(config.options.menu_timeout),
        bootloader_interface_timeouts.direct,
        loader_conf_timeout,
    ]
    .into_iter()
    .find(|timeout| !matches!(timeout, BootloaderInterfaceTimeout::Unspecified))
    .unwrap_or_default();

    // The menu timeout in seconds, if no source specified one.
    let mut menu_timeout = DEFAULT_MENU_TIMEOUT_SECONDS;

    // Whether the boot menu should wait for the user, instead of counting down.
    let mut wait_for_user = false;

    // Whether the menu is disabled, so that a key press can't bring it up.
    let mut menu_disabled = false;

    // Apply the chosen timeout.
    match timeout {
        BootloaderInterfaceTimeout::MenuForce => {
            // Force the boot menu, and wait for the user to choose an entry.
            force_boot_menu = true;
            wait_for_user = true;
        }

        BootloaderInterfaceTimeout::MenuForceTimeout(timeout) => {
            // Force the boot menu with the specified timeout.
            force_boot_menu = true;
            menu_timeout = timeout;
        }

        BootloaderInterfaceTimeout::MenuHidden => {
            // Hide the boot menu by setting the timeout to zero.
            menu_timeout = 0;
        }

        BootloaderInterfaceTimeout::MenuDisabled => {
            // Hide the boot menu, and don't let a key press show it.
            menu_timeout = 0;
            menu_disabled = true;
        }

        BootloaderInterfaceTimeout::Timeout(timeout) => {
            // Configure the timeout to the specified value.
            menu_timeout = timeout;
        }

        BootloaderInterfaceTimeout::Unspecified => {
            // Do nothing.
        }
    }

    // Whether the default entry is saved on every boot, which is what `@saved` asks for.
    // The saved entry is selected by `@saved` from the bootloader interface, or from loader.conf
    // when the bootloader interface does not set an entry.
    let is_saved = |value: &Option<String>| {
        value
            .as_deref()
            .is_some_and(|v| v.eq_ignore_ascii_case("@saved"))
    };
    let use_saved_entry = is_saved(&bootloader_interface_default_entry)
        || (bootloader_interface_default_entry.is_none() && is_saved(&loader_conf.default));

    // Turns the value of an entry setting into a pattern. `@saved` is the entry that was saved
    // by the previous boot, and any other value that starts with `@` is not supported.
    let entry_pattern = |value: Option<String>| -> Option<String> {
        let value = value?;
        if value.eq_ignore_ascii_case("@saved") {
            return last_booted_entry.clone();
        }
        if value.starts_with('@') {
            warn!("ignoring unsupported entry setting '{}'", value);
            return None;
        }
        Some(value)
    };

    // Pick the default entry from the first source that matches an entry. A source that matches
    // no entry, such as when the entry was removed, is ignored so that the next source is used.
    // The preferred entry sources never use an entry with no boot counter tries left.
    // Each source has whether it matches by id, and whether it skips bad entries.
    for (source, value, by_id, skip_bad) in [
        (
            "configuration",
            config.options.default_entry.clone(),
            false,
            false,
        ),
        (
            "bootloader interface preferred",
            bootloader_interface_preferred_entry.clone(),
            true,
            true,
        ),
        (
            "loader.conf preferred",
            loader_conf.preferred.clone(),
            true,
            true,
        ),
        (
            "bootloader interface default",
            bootloader_interface_default_entry.clone(),
            true,
            false,
        ),
        (
            "loader.conf default",
            loader_conf.default.clone(),
            true,
            false,
        ),
    ] {
        // The configuration is a pattern as it is. The other sources can be `@saved`.
        let pattern = if by_id { entry_pattern(value) } else { value };
        let Some(pattern) = pattern else {
            continue;
        };
        let matches = |entry: &BootableEntry| {
            let matched = if by_id {
                entry.is_match_id(&pattern)
            } else {
                entry.is_match(&pattern)
            };
            matched && !(skip_bad && entry.is_bad())
        };
        // A source that only matches entries with no boot counter tries left is skipped while
        // another source could pick a usable entry, unless every entry is bad.
        let any_usable = entries.iter().any(|entry| !entry.is_bad());
        if entries
            .iter()
            .any(|entry| matches(entry) && (!any_usable || !entry.is_bad()))
        {
            // Mark the matching entries as the default and unmark all the others.
            for entry in &mut entries {
                if matches(entry) {
                    entry.mark_default();
                } else {
                    entry.unmark_default();
                }
            }
            break;
        }
        warn!("ignoring {} entry '{}': no matching entry", source, pattern);
    }

    // Entries that have no boot counter tries left are never picked automatically.
    // If no entries are the default, pick the first usable entry as the default entry.
    let default_flags = edera_sprout_bls::resolve_default_flags(
        &entries
            .iter()
            .map(|entry| (entry.is_default(), entry.is_bad()))
            .collect::<Vec<_>>(),
    );
    for (entry, default) in entries.iter_mut().zip(default_flags) {
        if default {
            entry.mark_default();
        } else {
            entry.unmark_default();
        }
    }

    // Convert the menu timeout to a duration.
    // A zero timeout boots without showing the menu, so a forced menu with a zero timeout
    // waits for the user instead.
    let menu_timeout = if wait_for_user || (force_boot_menu && menu_timeout == 0) {
        None
    } else {
        Some(Duration::from_secs(menu_timeout))
    };

    // Determine the menu style based on the options or configuration.
    // We prefer the options over the configuration to allow for overriding.
    let menu_style = context
        .root()
        .options()
        .menu_style
        .unwrap_or(config.options.menu_style);

    // Find the forced boot entry, unless the boot menu is forced.
    // The oneshot entry from the bootloader interface is forced by id, and it is used instead of
    // --boot. If the forced boot entry can't be found, such as when it was removed,
    // the boot menu is used instead.
    let forced_entry = if force_boot_menu {
        None
    } else if let Some(ref oneshot) = bootloader_interface_oneshot_entry {
        let entry = BootableEntry::find_id(oneshot, entries.iter());
        if entry.is_none() {
            warn!("unable to find entry '{}', using the boot menu", oneshot);
        }
        entry
    } else {
        force_boot_entry.as_ref().and_then(|force_boot_entry| {
            let entry = BootableEntry::find(force_boot_entry, entries.iter());
            if entry.is_none() {
                warn!(
                    "unable to find entry '{}', using the boot menu",
                    force_boot_entry
                );
            }
            entry
        })
    };

    // Use the forced boot entry if possible, otherwise pick an entry using a boot menu.
    let entry = match forced_entry {
        Some(entry) => entry,
        // Delegate to the menu to select an entry to boot.
        None => menu::select(&timer, menu_timeout, menu_disabled, menu_style, &entries)
            .context("unable to select entry via boot menu")?,
    };

    // Tell the bootloader interface what the selected entry is.
    advisory(
        BootloaderInterface::set_selected_entry(entry.id())
            .context("unable to set selected entry in bootloader interface"),
    );

    // Save the selected entry for the next boot when `@saved` asks for it, and forget a saved
    // entry that is no longer used. A forced boot, such as a one-shot entry, changes nothing,
    // and neither does an unchanged value.
    if forced_entry.is_none() {
        let id = entry.id();
        if use_saved_entry {
            if !last_booted_entry
                .as_deref()
                .is_some_and(|saved| saved.eq_ignore_ascii_case(&id))
            {
                advisory(
                    BootloaderInterface::set_last_booted_entry(&id)
                        .context("unable to save last booted entry in bootloader interface"),
                );
            }
        } else if last_booted_entry.is_some() {
            advisory(
                BootloaderInterface::remove_last_booted_entry()
                    .context("unable to remove last booted entry in bootloader interface"),
            );
        }
    }

    // Execute the late phase, now that the entry is chosen but before its actions are executed.
    phase(context.clone(), &config.phases.late).context("unable to execute late phase")?;

    // Consume a try from the boot counter of the selected entry.
    // Failing to do so must not prevent the entry from booting.
    // An entry with no tries left, which was picked by hand, is counted too, so that it can
    // still be marked as good.
    if let Some(target) = entry.boot_counter() {
        match target.consume() {
            Ok(path) => {
                info!("updated boot counter of entry {}: {}", entry.name(), path);
                // Tell the system where the counter is, so it can mark the boot as good.
                let path = edera_sprout_bls::boot_path(&path.to_string());
                advisory(
                    BootloaderInterface::set_boot_count_path(&path)
                        .context("unable to set boot count path in bootloader interface"),
                );
            }
            Err(error) => warn!(
                "unable to update boot counter of entry {}: {:#}",
                entry.name(),
                error
            ),
        }
    }

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
#[entry]
fn efi_main() -> Status {
    // Initialize the basic UEFI environment.
    // If initialization fails, we will return ABORTED.
    // NOTE: This function will also initialize the logger.
    // The logger will panic if it is unable to initialize.
    // It is guaranteed that if this returns, the logger is initialized.
    if let Err(error) = setup::init() {
        error!("unable to initialize environment: {}", error);
        return Status::ABORTED;
    }

    // Run Sprout, then handle the error.
    let result = run();
    if let Err(ref error) = result {
        // Print an error trace.
        error!("sprout encountered an error: {}", error);
        for (index, stack) in error.chain().enumerate() {
            error!("[{}]: {}", index, stack);
        }
        // Sleep to allow the user to read the error.
        uefi::boot::stall(DELAY_ON_ERROR);
        return Status::ABORTED;
    }

    // Sprout doesn't necessarily guarantee anything was booted.
    // If we reach here, we will exit back to whoever called us.
    Status::SUCCESS
}

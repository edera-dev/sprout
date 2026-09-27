use crate::entries::BootableEntry;
use crate::menu::{BootMenu, read_key};
use anyhow::{Context, Result};
use core::time::Duration;
use log::info;
use uefi::proto::console::text::{Input, Key, ScanCode};

/// The characters that can be used to select an entry from keys.
const ENTRY_NUMBER_TABLE: &[char] = &['0', '1', '2', '3', '4', '5', '6', '7', '8', '9'];

/// A boot menu that prints the entries and selects them by number.
pub struct BasicMenu;

/// Represents the operation that can be performed by the boot menu.
#[derive(PartialEq, Eq)]
enum MenuOperation {
    /// The user selected a numbered entry.
    Number(usize),
    /// The user selected the escape key to exit the boot menu.
    Exit,
    /// The user selected the enter key to display the entries again.
    Continue,
    /// Timeout occurred.
    Timeout,
    /// No operation should be performed.
    Nop,
}

/// Read a key from the input device with an optional timeout, returning the [MenuOperation]
/// that was performed. Without a timeout, this waits for a key indefinitely.
fn read(input: &mut Input, timeout: Option<Duration>) -> Result<MenuOperation> {
    // If the timer triggered, the user did not select a numbered entry.
    let Some(key) = read_key(input, timeout)? else {
        return Ok(MenuOperation::Timeout);
    };

    match key {
        Key::Printable(c) => {
            // If the key is not ascii, we can't process it.
            if !c.is_ascii() {
                return Ok(MenuOperation::Continue);
            }
            // Convert the key to a char.
            let c: char = c.into();
            // Find the key pressed in the entry number table or continue.
            Ok(ENTRY_NUMBER_TABLE
                .iter()
                .position(|&x| x == c)
                .map(MenuOperation::Number)
                .unwrap_or(MenuOperation::Continue))
        }

        // The escape key is used to exit the boot menu.
        Key::Special(ScanCode::ESCAPE) => Ok(MenuOperation::Exit),

        // If the special key is unknown, do nothing.
        Key::Special(_) => Ok(MenuOperation::Nop),
    }
}

/// Find the default entry in `entries`.
fn default_entry(entries: &[BootableEntry]) -> Result<&BootableEntry> {
    entries
        .iter()
        .find(|item| item.is_default())
        .context("no default entry available")
}

/// Selects an entry from the list of entries using the boot menu.
fn select_with_input<'a>(
    input: &mut Input,
    timeout: Duration,
    entries: &'a [BootableEntry],
) -> Result<&'a BootableEntry> {
    // If the timeout is zero, boot the default entry without showing the menu.
    if timeout.is_zero() {
        return default_entry(entries);
    }

    // The time to wait for a key, or None once a key stops the countdown.
    let mut countdown = Some(timeout);

    loop {
        // Print all the entries with the number used to select them.
        info!("Boot Menu:");
        for (index, entry) in entries.iter().enumerate() {
            let title = entry.context().stamp(&entry.declaration().title);
            info!("  [{}] {}", index, title);
        }

        // Read from input until a valid operation is selected.
        let operation = loop {
            info!("Select a boot entry using the number keys.");
            info!("Press Escape to exit and enter to display the entries again.");

            let operation = read(input, countdown)?;

            // Any key stops the countdown, so the menu waits for the user from now on.
            if operation != MenuOperation::Timeout {
                countdown = None;
            }

            if operation != MenuOperation::Nop {
                break operation;
            }
        };

        match operation {
            // Entry was selected by number. If the number is invalid, we continue.
            MenuOperation::Number(index) => {
                let Some(entry) = entries.get(index) else {
                    info!("invalid entry number");
                    continue;
                };
                return Ok(entry);
            }

            // When the user exits the boot menu or a timeout occurs, we should
            // boot the default entry, if any.
            MenuOperation::Exit | MenuOperation::Timeout => {
                return default_entry(entries);
            }

            // If the operation is to continue or nop, we can just run the loop again.
            MenuOperation::Continue | MenuOperation::Nop => {
                continue;
            }
        }
    }
}

impl BootMenu for BasicMenu {
    /// Selects an entry using the basic menu.
    /// The actual work is done internally in [select_with_input] which is called
    /// within the context of the standard input device.
    fn select<'a>(
        &self,
        timeout: Duration,
        entries: &'a [BootableEntry],
    ) -> Result<&'a BootableEntry> {
        uefi::system::with_stdin(move |input| select_with_input(input, timeout, entries))
    }
}

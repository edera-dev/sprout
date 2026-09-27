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
    /// The user typed a digit of an entry number.
    Digit(usize),
    /// The user selected the escape key to exit the boot menu.
    Exit,
    /// The user selected the enter key to boot the typed entry number,
    /// or to display the entries again if no number was typed.
    Enter,
    /// The user selected some other key to display the entries again.
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
            // Serial consoles may send a line feed instead of a carriage return.
            if matches!(c, '\r' | '\n') {
                return Ok(MenuOperation::Enter);
            }
            // Find the key pressed in the entry number table or continue.
            Ok(ENTRY_NUMBER_TABLE
                .iter()
                .position(|&x| x == c)
                .map(MenuOperation::Digit)
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
    timeout: Option<Duration>,
    entries: &'a [BootableEntry],
) -> Result<&'a BootableEntry> {
    // If the timeout is zero, boot the default entry without showing the menu.
    if timeout.is_some_and(|timeout| timeout.is_zero()) {
        return default_entry(entries);
    }

    // The time to wait for a key, or None once a key stops the countdown.
    let mut countdown = timeout;

    // The entry number typed so far, while more digits could still select another entry.
    let mut typed: Option<usize> = None;
    // Whether to print the entries before reading the next key.
    let mut display = true;

    loop {
        if display {
            // Print all the entries with the number used to select them.
            info!("Boot Menu:");
            for (index, entry) in entries.iter().enumerate() {
                let title = entry.context().stamp(&entry.declaration().title);
                info!("  [{}] {}", index, title);
            }

            info!("Select a boot entry using the number keys.");
            if entries.len() > ENTRY_NUMBER_TABLE.len() {
                info!("Press enter after the number if the entry does not boot right away.");
            }
            info!("Press Escape to exit and enter to display the entries again.");
            display = false;
        }

        let operation = read(input, countdown)?;

        // Any key stops the countdown, so the menu waits for the user from now on.
        if operation != MenuOperation::Timeout {
            countdown = None;
        }

        match operation {
            // A digit of the entry number was typed.
            MenuOperation::Digit(digit) => {
                let number = typed
                    .take()
                    .unwrap_or(0)
                    .saturating_mul(10)
                    .saturating_add(digit);

                // If another digit could select a different entry, wait for it or for enter.
                if number != 0 && number.saturating_mul(10) < entries.len() {
                    typed = Some(number);
                    continue;
                }

                // Otherwise, boot the entry. If the number is invalid, we continue.
                let Some(entry) = entries.get(number) else {
                    info!("invalid entry number");
                    continue;
                };
                return Ok(entry);
            }

            // Enter boots the typed entry number, or displays the entries again.
            MenuOperation::Enter => {
                let Some(number) = typed.take() else {
                    display = true;
                    continue;
                };
                let Some(entry) = entries.get(number) else {
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

            // Any other key clears the typed number and displays the entries again.
            MenuOperation::Continue => {
                typed = None;
                display = true;
            }

            // If the operation is nop, there is nothing to do.
            MenuOperation::Nop => {}
        }
    }
}

impl BootMenu for BasicMenu {
    /// Selects an entry using the basic menu.
    /// The actual work is done internally in [select_with_input] which is called
    /// within the context of the standard input device.
    fn select<'a>(
        &self,
        timeout: Option<Duration>,
        entries: &'a [BootableEntry],
    ) -> Result<&'a BootableEntry> {
        uefi::system::with_stdin(move |input| select_with_input(input, timeout, entries))
    }
}

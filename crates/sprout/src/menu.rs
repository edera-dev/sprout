use crate::entries::BootableEntry;
use crate::integrations::bootloader_interface::BootloaderInterface;
use crate::platform::timer::PlatformTimer;
use alloc::vec;
use anyhow::{Context, Result, bail};
use core::time::Duration;
use log::{info, warn};
use uefi::ResultExt;
use uefi::boot::TimerTrigger;
use uefi::proto::console::text::{Input, Key, ScanCode};
use uefi_raw::table::boot::{EventType, Tpl};

/// The characters that can be used to select an entry from keys.
const ENTRY_NUMBER_TABLE: &[char] = &['0', '1', '2', '3', '4', '5', '6', '7', '8', '9'];

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

/// Read a key from the input device with a duration, returning the [MenuOperation] that was
/// performed.
fn read(input: &mut Input, timeout: &Duration) -> Result<MenuOperation> {
    // The event to wait for a key press.
    let key_event = input
        .wait_for_key_event()
        .context("unable to acquire key event")?;

    // Timer event for timeout.
    // SAFETY: The timer event creation allocated a timer pointer on the UEFI heap.
    // This is validated safe as long as we are in boot services.
    let timer_event = unsafe {
        uefi::boot::create_event_ex(EventType::TIMER, Tpl::CALLBACK, None, None, None)
            .context("unable to create timer event")?
    };

    // The timeout is in increments of 100 nanoseconds.
    let timeout_hundred_nanos = timeout.as_nanos() / 100;

    // Check if the timeout is too large to fit into an u64.
    if timeout_hundred_nanos > u64::MAX as u128 {
        bail!("timeout duration overflow");
    }

    // Set a timer to trigger after the specified duration.
    let trigger = TimerTrigger::Relative(timeout_hundred_nanos as u64);
    uefi::boot::set_timer(&timer_event, trigger).context("unable to set timeout timer")?;

    let mut events = vec![timer_event, key_event];

    // Wait for either the timer event or the key event to trigger.
    // Store the result so that we can free the timer event.
    let event_result = uefi::boot::wait_for_event(&mut events)
        .discard_errdata()
        .context("unable to wait for event");

    // Close the timer event that we acquired.
    // We don't need to close the key event because it is owned globally.
    // This should always be called in practice as events are not modified by wait_for_event.
    if let Some(timer_event) = events.into_iter().next() {
        // Store the result of the close event so we can determine if we can safely assert it.
        let close_event_result =
            uefi::boot::close_event(timer_event).context("unable to close timer event");
        if event_result.is_err()
            && let Err(ref close_event_error) = close_event_result
        {
            // Log a warning if we failed to close the timer event.
            // This is done to ensure we don't mask the wait_for_event error.
            warn!("unable to close timer event: {}", close_event_error);
        } else {
            // If we reach here, we can safely assert that the close event succeeded without
            // masking the wait_for_event error.
            close_event_result?;
        }
    }

    // Acquire the event that triggered.
    let event = event_result?;

    // The first event is the timer event.
    // If it has triggered, the user did not select a numbered entry.
    if event == 0 {
        return Ok(MenuOperation::Timeout);
    }

    // If we reach here, there is a key event.
    let Some(key) = input.read_key().context("unable to read key")? else {
        bail!("no key was pressed");
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

/// Selects an entry from the list of entries using the boot menu.
fn select_with_input<'a>(
    input: &mut Input,
    timeout: Duration,
    entries: &'a [BootableEntry],
) -> Result<&'a BootableEntry> {
    loop {
        // If the timeout is not zero, let's display the boot menu.
        if !timeout.is_zero() {
            // Until a pretty menu is available, we just print all the entries.
            info!("Boot Menu:");
            for (index, entry) in entries.iter().enumerate() {
                let title = entry.context().stamp(&entry.declaration().title);
                info!("  [{}] {}", index, title);
            }
        }

        // Read from input until a valid operation is selected.
        let operation = loop {
            // If the timeout is zero, we can exit immediately because there is nothing to do.
            if timeout.is_zero() {
                break MenuOperation::Exit;
            }

            info!("Select a boot entry using the number keys.");
            info!("Press Escape to exit and enter to display the entries again.");

            let operation = read(input, &timeout)?;
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
                return entries
                    .iter()
                    .find(|item| item.is_default())
                    .context("no default entry available");
            }

            // If the operation is to continue or nop, we can just run the loop again.
            MenuOperation::Continue | MenuOperation::Nop => {
                continue;
            }
        }
    }
}

/// Shows a boot menu to select a bootable entry to boot.
/// The actual work is done internally in [select_with_input] which is called
/// within the context of the standard input device.
pub fn select<'live>(
    timer: &'live PlatformTimer,
    timeout: Duration,
    entries: &'live [BootableEntry],
) -> Result<&'live BootableEntry> {
    // Notify the bootloader interface that we are about to display the menu.
    BootloaderInterface::mark_menu(timer)
        .context("unable to mark menu display in bootloader interface")?;

    // Acquire the standard input device and run the boot menu.
    uefi::system::with_stdin(move |input| select_with_input(input, timeout, entries))
}

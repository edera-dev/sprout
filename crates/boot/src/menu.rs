use crate::entries::BootableEntry;
use crate::menu::basic::BasicMenu;
use crate::menu::simple::SimpleMenu;
use alloc::vec;
use anyhow::{Context, Result};
use core::time::Duration;
use edera_sprout_config::MenuStyle;
use eficore::bootloader_interface::BootloaderInterface;
use eficore::platform::timer::PlatformTimer;
use log::warn;
use uefi::ResultExt;
use uefi::boot::TimerTrigger;
use uefi::proto::console::text::Input;
use uefi_raw::table::boot::{EventType, Tpl};

/// basic: A menu that prints the entries and selects them by number.
pub mod basic;

/// simple: A full-screen menu that selects entries with the arrow keys.
pub mod simple;

/// The longest duration a single timer is set for. This is well within the range of
/// the 100ns units that timers are set in, and still over a century long.
const MAX_TIMER_DURATION: Duration = Duration::from_secs(u32::MAX as u64);

/// A boot menu that can be shown to select an entry to boot.
pub trait BootMenu {
    /// Select an entry from `entries` to boot. If no entry is chosen before `timeout` passes,
    /// the default entry is selected.
    fn select<'a>(
        &self,
        timeout: Duration,
        entries: &'a [BootableEntry],
    ) -> Result<&'a BootableEntry>;
}

/// Wait for the key event of `input` to be signaled, giving up once `timeout` passes.
/// Returns true if the key event was signaled, or false if the timeout passed.
/// Some firmware signals the key event without a key being available, so reading
/// the key afterward may still produce no key.
pub fn wait_for_key(input: &mut Input, timeout: Duration) -> Result<bool> {
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

    // Set a timer to trigger after the specified duration.
    // The timer is limited to what the firmware can represent, as a longer timeout
    // can't be converted into a timer trigger.
    let trigger = TimerTrigger::Relative(timeout.min(MAX_TIMER_DURATION));
    uefi::boot::set_timer(&timer_event, trigger).context("unable to set timeout timer")?;

    let events = vec![timer_event, key_event];

    // Wait for either the timer event or the key event to trigger.
    // Store the result so that we can free the timer event.
    let event_result = uefi::boot::wait_for_event(&events)
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

    // The first event is the timer event, so any other event is the key event.
    Ok(event != 0)
}

/// Shows a boot menu of the specified `style` to select a bootable entry to boot.
pub fn select<'live>(
    timer: &'live PlatformTimer,
    timeout: Duration,
    style: MenuStyle,
    entries: &'live [BootableEntry],
) -> Result<&'live BootableEntry> {
    // Notify the bootloader interface that we are about to display the menu.
    BootloaderInterface::mark_menu(timer)
        .context("unable to mark menu display in bootloader interface")?;

    // Pick the menu that implements the requested style.
    let menu: &dyn BootMenu = match style {
        MenuStyle::Basic => &BasicMenu,
        MenuStyle::Simple => &SimpleMenu,
    };

    menu.select(timeout, entries)
}

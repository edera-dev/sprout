use crate::entries::BootableEntry;
use crate::menu::basic::BasicMenu;
use crate::menu::graphical::GraphicalMenu;
use crate::menu::simple::SimpleMenu;
use alloc::vec;
use alloc::vec::Vec;
use anyhow::{Context, Result};
use core::time::Duration;
use edera_sprout_config::MenuStyle;
use eficore::bootloader_interface::BootloaderInterface;
use eficore::platform::timer::PlatformTimer;
use log::warn;
use uefi::Event;
use uefi::ResultExt;
use uefi::boot::TimerTrigger;
use uefi::proto::console::text::{Input, Key};
use uefi_raw::table::boot::{EventType, Tpl};

/// basic: A menu that prints the entries and selects them by number.
pub mod basic;

/// font: The bitmap font that the graphical menu draws text with.
mod font;

/// graphical: A menu drawn on the graphics output that is used with the keyboard, or the mouse if enabled.
pub mod graphical;

/// logo: The Sprout logo that the graphical menu draws in a corner.
mod logo;

/// simple: A full-screen menu that selects entries with the arrow keys.
pub mod simple;

/// How long a hidden menu waits for a key press before the default entry is booted.
const HIDDEN_MENU_KEY_WAIT: Duration = Duration::from_millis(100);

/// The longest duration a single timer is set for. This is well within the range of
/// the 100ns units that timers are set in, and still over a century long.
const MAX_TIMER_DURATION: Duration = Duration::from_secs(u32::MAX as u64);

/// A boot menu that can be shown to select an entry to boot.
pub trait BootMenu {
    /// Select an entry from `entries` to boot. If no entry is chosen before `timeout` passes,
    /// the default entry is selected. If `timeout` is zero, the default entry is selected
    /// without showing the menu. If `timeout` is [None], the menu waits for the user.
    fn select<'a>(
        &self,
        timeout: Option<Duration>,
        entries: &'a [BootableEntry],
    ) -> Result<&'a BootableEntry>;
}

/// Read a key from `input`, giving up once `timeout` passes.
/// If `timeout` is [None], this waits for a key indefinitely.
/// Returns the key that was pressed, or [None] if the timeout passed.
pub fn read_key(input: &mut Input, timeout: Option<Duration>) -> Result<Option<Key>> {
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

    let events = vec![timer_event, key_event];

    // Wait for a key until the timer triggers.
    // Store the result so that we can free the timer event.
    let result = (|| {
        // Set a timer to trigger after the specified duration.
        // The timer is limited to what the firmware can represent, as a longer timeout
        // can't be converted into a timer trigger.
        // Without a timeout, the timer is never set, so it never triggers.
        if let Some(timeout) = timeout {
            let trigger = TimerTrigger::Relative(timeout.min(MAX_TIMER_DURATION));
            uefi::boot::set_timer(&events[0], trigger).context("unable to set timeout timer")?;
        }

        loop {
            // Wait for either the timer event or the key event to trigger.
            let event = uefi::boot::wait_for_event(&events)
                .discard_errdata()
                .context("unable to wait for event")?;

            // The first event is the timer event, so the timeout has passed.
            if event == 0 {
                return Ok(None);
            }

            // Some firmware signals the key event without a key being available.
            // In that case, keep waiting on the same timer so the timeout is not extended.
            if let Some(key) = input.read_key().context("unable to read key")? {
                return Ok(Some(key));
            }
        }
    })();

    // Close the timer event that we acquired.
    // We don't need to close the key event because it is owned globally.
    // This should always be called in practice as events are not modified by wait_for_event.
    if let Some(timer_event) = events.into_iter().next() {
        // Store the result of the close event so we can determine if we can safely assert it.
        let close_event_result =
            uefi::boot::close_event(timer_event).context("unable to close timer event");
        if result.is_err()
            && let Err(ref close_event_error) = close_event_result
        {
            // Log a warning if we failed to close the timer event.
            // This is done to ensure we don't mask the error from reading the key.
            warn!("unable to close timer event: {}", close_event_error);
        } else {
            // If we reach here, we can safely assert that the close event succeeded without
            // masking the error from reading the key.
            close_event_result?;
        }
    }

    result
}

/// Wait until one of `events` is signaled, giving up once `timeout` passes.
/// If `timeout` is [None], this waits for an event indefinitely.
/// Returns the index of the event that was signaled, or [None] if the timeout passed.
pub fn wait_for_events(events: &[Event], timeout: Option<Duration>) -> Result<Option<usize>> {
    // Timer event for timeout.
    // SAFETY: The timer event creation allocated a timer pointer on the UEFI heap.
    // This is validated safe as long as we are in boot services.
    let timer_event = unsafe {
        uefi::boot::create_event_ex(EventType::TIMER, Tpl::CALLBACK, None, None, None)
            .context("unable to create timer event")?
    };

    // The timer is the last event so the indexes of the events that were passed are kept.
    // The events are only borrowed, so the timer is the only one that is closed.
    // SAFETY: The cloned event is only used to wait, and is not closed.
    let mut all: Vec<Event> = events
        .iter()
        .map(|event| unsafe { event.unsafe_clone() })
        .collect();
    all.push(unsafe { timer_event.unsafe_clone() });

    let result = (|| {
        // Without a timeout, the timer is never set, so it never triggers.
        if let Some(timeout) = timeout {
            let trigger = TimerTrigger::Relative(timeout.min(MAX_TIMER_DURATION));
            uefi::boot::set_timer(&timer_event, trigger).context("unable to set timeout timer")?;
        }

        let index = uefi::boot::wait_for_event(&all)
            .discard_errdata()
            .context("unable to wait for event")?;
        Ok((index != events.len()).then_some(index))
    })();

    // Close the timer event that we acquired, without masking an error from waiting.
    if let Err(error) = uefi::boot::close_event(timer_event) {
        if result.is_err() {
            warn!("unable to close timer event: {}", error);
        } else {
            return Err(error).context("unable to close timer event");
        }
    }

    result
}

/// Shows a boot menu of the specified `style` to select a bootable entry to boot.
/// See [BootMenu::select] for how `timeout` is handled. A zero `timeout` hides the menu, but
/// the menu is still shown if a key is pressed right away, unless `menu_disabled` is set.
pub fn select<'live>(
    timer: &'live PlatformTimer,
    timeout: Option<Duration>,
    menu_disabled: bool,
    style: MenuStyle,
    graphical_mouse: bool,
    entries: &'live [BootableEntry],
) -> Result<&'live BootableEntry> {
    // A hidden menu gives the user a moment to ask for the menu with a key press.
    let timeout = if timeout.is_some_and(|timeout| timeout.is_zero()) && !menu_disabled {
        match uefi::system::with_stdin(|input| read_key(input, Some(HIDDEN_MENU_KEY_WAIT))) {
            Ok(Some(_)) => None,
            Ok(None) => timeout,
            Err(error) => {
                warn!("unable to check for a key press: {:#}", error);
                timeout
            }
        }
    } else {
        timeout
    };

    // Notify the bootloader interface that we are about to display the menu.
    // This is only informational, so it should not prevent booting.
    if let Err(error) = BootloaderInterface::mark_menu(timer) {
        warn!(
            "unable to mark menu display in bootloader interface: {:#}",
            error
        );
    }

    // Pick the menu that implements the requested style.
    match style {
        MenuStyle::Basic => BasicMenu.select(timeout, entries),

        // The graphical menu needs a graphics output, which not every machine has.
        // If it fails, the simple menu is used instead, which falls back to the basic menu.
        MenuStyle::Graphical => GraphicalMenu {
            enable_mouse: graphical_mouse,
        }
        .select(timeout, entries)
        .or_else(|error| {
            warn!(
                "unable to show the graphical boot menu, using the simple menu: {:#}",
                error
            );
            select_simple(timeout, entries)
        }),

        // The simple menu needs a console that can move the cursor and set colors, which
        // not every console supports. If it fails, the basic menu is used instead.
        MenuStyle::Simple => select_simple(timeout, entries),
    }
}

/// Shows the simple boot menu, or the basic boot menu if the console does not support it.
/// See [BootMenu::select] for how `timeout` is handled.
fn select_simple(timeout: Option<Duration>, entries: &[BootableEntry]) -> Result<&BootableEntry> {
    SimpleMenu.select(timeout, entries).or_else(|error| {
        warn!(
            "unable to show the simple boot menu, using the basic menu: {:#}",
            error
        );
        BasicMenu.select(timeout, entries)
    })
}

use crate::entries::BootableEntry;
use crate::menu::{BootMenu, read_key};
use alloc::format;
use alloc::string::String;
use anyhow::{Context, Result, anyhow};
use core::fmt::Write;
use core::iter::repeat;
use core::time::Duration;
use uefi::proto::console::text::{Color, Input, Key, Output, ScanCode};

/// The number of columns to assume if the console does not report its mode.
const FALLBACK_COLUMNS: usize = 80;

/// The number of rows to assume if the console does not report its mode.
const FALLBACK_ROWS: usize = 25;

/// How often the countdown is updated.
const TICK: Duration = Duration::from_secs(1);

/// The number of spaces on either side of an entry title.
const ENTRY_PADDING: usize = 2;

/// The hint shown once the countdown has been stopped.
const HINT: &str = "Use the arrow keys to select an entry and press enter to boot.";

/// A full-screen boot menu that selects entries with the arrow keys, similar to systemd-boot.
pub struct SimpleMenu;

/// The position and size of the menu on the console.
struct Layout {
    /// The number of rows on the console.
    rows: usize,
    /// The number of columns that can be drawn to.
    columns: usize,
    /// The column that entries start at.
    x: usize,
    /// The row that entries start at.
    y: usize,
    /// The width of each entry, including padding.
    width: usize,
    /// The number of entries that fit on the console.
    visible: usize,
}

impl Layout {
    /// Calculate the layout of `entries` on the console `output`.
    fn new(output: &Output, entries: &[BootableEntry]) -> Self {
        // Some consoles do not report a mode, so we assume a standard size for them.
        let (columns, rows) = output
            .current_mode()
            .ok()
            .flatten()
            .map(|mode| (mode.columns(), mode.rows()))
            .unwrap_or((FALLBACK_COLUMNS, FALLBACK_ROWS));

        // Leave the last column empty, as writing to it makes some consoles scroll.
        let columns = columns.saturating_sub(1);

        // All entries are the width of the longest title so the highlight lines up.
        let longest = entries
            .iter()
            .map(|entry| entry.title().chars().count())
            .max()
            .unwrap_or(0);
        let width = (longest + ENTRY_PADDING * 2).min(columns);

        // Keep the last two rows for the status line and a blank row above it.
        let space = rows.saturating_sub(2).max(1);
        let visible = entries.len().min(space);

        Self {
            rows,
            columns,
            x: (columns - width) / 2,
            y: (space - visible) / 2,
            width,
            visible,
        }
    }
}

/// Draw `text` at `column` and `row`, padded with spaces or cut to fit `width`.
fn draw(
    output: &mut Output,
    column: usize,
    row: usize,
    width: usize,
    text: &str,
    colors: (Color, Color),
) -> Result<()> {
    // Replace anything that isn't printable ASCII, as not all consoles can draw it.
    let line: String = text
        .chars()
        .map(|c| {
            if c == ' ' || c.is_ascii_graphic() {
                c
            } else {
                '?'
            }
        })
        .chain(repeat(' '))
        .take(width)
        .collect();

    output
        .set_color(colors.0, colors.1)
        .context("unable to set console color")?;
    output
        .set_cursor_position(column, row)
        .context("unable to set console cursor position")?;
    output
        .write_str(&line)
        .map_err(|_| anyhow!("unable to write to console"))?;
    Ok(())
}

/// Draw the visible `entries` starting at `offset` with `selected` highlighted,
/// and the `status` line at the bottom of the console.
fn render(
    output: &mut Output,
    layout: &Layout,
    entries: &[BootableEntry],
    selected: usize,
    offset: usize,
    status: &str,
) -> Result<()> {
    for (row, (index, entry)) in entries
        .iter()
        .enumerate()
        .skip(offset)
        .take(layout.visible)
        .enumerate()
    {
        // The selected entry is drawn in inverted colors.
        let colors = if index == selected {
            (Color::Black, Color::LightGray)
        } else {
            (Color::LightGray, Color::Black)
        };
        let text = format!("{:padding$}{}", "", entry.title(), padding = ENTRY_PADDING);
        draw(
            output,
            layout.x,
            layout.y + row,
            layout.width,
            &text,
            colors,
        )?;
    }

    // Center the status line across the console.
    let padding = layout.columns.saturating_sub(status.len()) / 2;
    let text = format!("{:padding$}{}", "", status);
    draw(
        output,
        0,
        layout.rows.saturating_sub(1),
        layout.columns,
        &text,
        (Color::LightGray, Color::Black),
    )
}

/// Run the menu until an entry is chosen, returning the index of that entry.
/// The `selected` entry is booted if no key is pressed before `timeout` passes.
/// Without a timeout, the menu waits for the user.
fn run(
    input: &mut Input,
    output: &mut Output,
    layout: &Layout,
    timeout: Option<Duration>,
    entries: &[BootableEntry],
    mut selected: usize,
) -> Result<usize> {
    let last = entries.len() - 1;
    // The first visible entry, which changes when scrolling through a long list.
    let mut offset = 0;
    // The time left before booting, or None once a key stops the countdown.
    let mut remaining = timeout;

    loop {
        // Scroll so that the selected entry is visible.
        if selected < offset {
            offset = selected;
        } else if selected >= offset + layout.visible {
            offset = selected + 1 - layout.visible;
        }

        let status = match remaining {
            Some(remaining) => format!("Boot in {} s.", remaining.as_secs()),
            None => String::from(HINT),
        };
        render(output, layout, entries, selected, offset, &status)?;

        // Wait for a key, waking up every tick to update the countdown.
        let tick = remaining.map_or(TICK, |remaining| remaining.min(TICK));
        let Some(key) = read_key(input, Some(tick))? else {
            // Without a countdown there's nothing to do until a key is pressed.
            let Some(left) = remaining else {
                continue;
            };

            let left = left.saturating_sub(tick);
            if left.is_zero() {
                return Ok(selected);
            }
            remaining = Some(left);
            continue;
        };

        // Any key stops the countdown, which leaves escape with nothing else to do.
        remaining = None;

        match key {
            Key::Special(ScanCode::UP) => selected = selected.saturating_sub(1),
            Key::Special(ScanCode::DOWN) => selected = (selected + 1).min(last),
            Key::Special(ScanCode::HOME) => selected = 0,
            Key::Special(ScanCode::END) => selected = last,
            Key::Special(ScanCode::PAGE_UP) => selected = selected.saturating_sub(layout.visible),
            Key::Special(ScanCode::PAGE_DOWN) => selected = (selected + layout.visible).min(last),

            // Serial consoles may send a line feed instead of a carriage return.
            Key::Printable(c) if matches!(char::from(c), '\r' | '\n') => return Ok(selected),

            _ => {}
        }
    }
}

/// Selects an entry from `entries` using the console `input` and `output`.
fn select_with_console<'a>(
    input: &mut Input,
    output: &mut Output,
    timeout: Option<Duration>,
    entries: &'a [BootableEntry],
) -> Result<&'a BootableEntry> {
    let default = entries
        .iter()
        .position(|entry| entry.is_default())
        .context("no default entry available")?;

    // If the timeout is zero, boot the default entry without showing the menu.
    if timeout.is_some_and(|timeout| timeout.is_zero()) {
        return Ok(&entries[default]);
    }

    let layout = Layout::new(output, entries);
    let cursor_visible = output.cursor_visible();

    output
        .set_color(Color::LightGray, Color::Black)
        .context("unable to set console color")?;
    output.clear().context("unable to clear console")?;
    // Not all consoles can hide the cursor, so this is allowed to fail.
    let _ = output.enable_cursor(false);

    let result = run(input, output, &layout, timeout, entries, default);

    // Put the console back so that anything printed after the menu is readable.
    let _ = output.set_color(Color::LightGray, Color::Black);
    let _ = output.clear();
    let _ = output.enable_cursor(cursor_visible);

    Ok(&entries[result?])
}

impl BootMenu for SimpleMenu {
    /// Selects an entry using the simple menu.
    /// The actual work is done internally in [select_with_console] which is called
    /// within the context of the standard input and output devices.
    fn select<'a>(
        &self,
        timeout: Option<Duration>,
        entries: &'a [BootableEntry],
    ) -> Result<&'a BootableEntry> {
        uefi::system::with_stdin(|input| {
            uefi::system::with_stdout(|output| select_with_console(input, output, timeout, entries))
        })
    }
}

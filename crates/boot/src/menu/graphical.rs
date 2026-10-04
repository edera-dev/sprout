use crate::entries::BootableEntry;
use crate::menu::font::{self, GLYPH_SIZE, Glyph, HEART};
use crate::menu::logo::{LOGO, LOGO_HEIGHT, LOGO_WIDTH};
use crate::menu::{BootMenu, wait_for_events};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use anyhow::{Context, Result, bail};
use core::time::Duration;
use eficore::framebuffer::Framebuffer;
use uefi::boot::{OpenProtocolAttributes, OpenProtocolParams, ScopedProtocol};
use uefi::proto::ProtocolPointer;
use uefi::proto::console::gop::{BltPixel, GraphicsOutput};
use uefi::proto::console::pointer::{AbsolutePointer, Pointer};
use uefi::proto::console::text::{Input, Key, ScanCode};
use uefi::proto::device_path::DevicePath;
use uefi::{Event, Handle};

/// How often the countdown and the logo are updated.
const TICK: Duration = Duration::from_millis(125);

/// The number of ticks in one bounce of the logo.
const BOUNCE_TICKS: usize = 16;

/// How many pixels the cursor moves for each millimeter that a relative pointer reports.
const PIXELS_PER_MILLIMETER: i64 = 8;

/// The title shown above the entries.
const TITLE: &str = "sprout";

/// The hint shown once the countdown has been stopped.
const HINT: &str = "Pick an entry with the arrow keys or the mouse.";

/// The color of the top of the background, which fades into [BACKGROUND_BOTTOM].
const BACKGROUND_TOP: (u8, u8, u8) = (0x3a, 0x1c, 0x4e);

/// The color of the bottom of the background.
const BACKGROUND_BOTTOM: (u8, u8, u8) = (0x16, 0x0c, 0x24);

/// The color of the pill behind each entry that isn't selected.
const PILL: BltPixel = BltPixel::new(0x2c, 0x1a, 0x40);

/// The color of the pill behind the selected entry, taken from the Sprout logo.
const PILL_SELECTED: BltPixel = BltPixel::new(0xff, 0x7a, 0xb8);

/// The color of the title and the heart.
const ACCENT: BltPixel = BltPixel::new(0xff, 0xa6, 0xd0);

/// The color of entries that aren't selected.
const TEXT: BltPixel = BltPixel::new(0xf3, 0xd9, 0xea);

/// The color of the selected entry.
const TEXT_SELECTED: BltPixel = BltPixel::new(0x3a, 0x10, 0x2c);

/// The color of the status line.
const TEXT_MUTED: BltPixel = BltPixel::new(0xb9, 0x9a, 0xc9);

/// The color that the cursor is filled with.
const CURSOR_FILL: BltPixel = BltPixel::new(0xff, 0xff, 0xff);

/// The color of the outline of the cursor.
const CURSOR_OUTLINE: BltPixel = BltPixel::new(0x3a, 0x10, 0x2c);

/// The shape of the cursor, where `X` is the outline and `.` is the fill.
const CURSOR: [&str; 16] = [
    "X          ",
    "XX         ",
    "X.X        ",
    "X..X       ",
    "X...X      ",
    "X....X     ",
    "X.....X    ",
    "X......X   ",
    "X.......X  ",
    "X........X ",
    "X.....XXXXX",
    "X..X..X    ",
    "X.X X..X   ",
    "XX  X..X   ",
    "X    X..X  ",
    "     XXXX  ",
];

/// A graphical boot menu that selects entries with the keyboard or the mouse.
pub struct GraphicalMenu;

/// The pointing devices of the firmware.
/// Firmware commonly has both kinds of protocol, even if one has no device behind it.
struct Mouse {
    /// A device that reports how far it has moved, like a mouse.
    relative: Option<ScopedProtocol<Pointer>>,
    /// A device that reports where it is, like a touch screen or the tablet of a virtual machine.
    absolute: Option<ScopedProtocol<AbsolutePointer>>,
}

/// Open the protocol `P` on `handle` without taking it from the firmware.
/// An exclusive open makes the firmware disconnect every driver that is using the protocol,
/// and on some firmware that never returns.
fn open_shared<P: ProtocolPointer + ?Sized>(handle: Handle) -> uefi::Result<ScopedProtocol<P>> {
    // SAFETY: The protocols opened this way are only used from this thread, and the firmware
    // keeps them installed for as long as the menu is open.
    unsafe {
        uefi::boot::open_protocol::<P>(
            OpenProtocolParams {
                handle,
                agent: uefi::boot::image_handle(),
                controller: None,
            },
            OpenProtocolAttributes::GetProtocol,
        )
    }
}

/// Open the pointer protocol `P`, preferring the one on the console input handle.
/// That one belongs to the console splitter, which combines every device and is never removed.
/// Otherwise, the first device that has the protocol is used.
fn open_pointer<P: ProtocolPointer + ?Sized>() -> Option<ScopedProtocol<P>> {
    // SAFETY: The handle in the system table is either null or a valid handle.
    let console = uefi::table::system_table_raw()
        .and_then(|table| unsafe { Handle::from_ptr(table.as_ref().stdin_handle) });
    console
        .and_then(|handle| open_shared::<P>(handle).ok())
        .or_else(|| {
            uefi::boot::get_handle_for_protocol::<P>()
                .and_then(open_shared::<P>)
                .ok()
        })
}

impl Mouse {
    /// Open the pointing devices, if the firmware has any.
    fn open() -> Option<Self> {
        let relative = open_pointer::<Pointer>();
        let absolute = open_pointer::<AbsolutePointer>();
        (relative.is_some() || absolute.is_some()).then_some(Self { relative, absolute })
    }

    /// The events that are signaled when a device has input.
    fn events(&self) -> Result<Vec<Event>> {
        let mut events = Vec::new();
        if let Some(pointer) = &self.relative {
            events.push(pointer.wait_for_input_event());
        }
        if let Some(pointer) = &self.absolute {
            events.push(pointer.wait_for_input_event());
        }
        events
            .into_iter()
            .collect::<uefi::Result<_>>()
            .context("unable to acquire pointer event")
    }

    /// Move `cursor` within `bounds` to where the devices say it is.
    /// Returns whether the button is held, or [None] if the devices have no new input.
    fn poll(
        &mut self,
        cursor: &mut (usize, usize),
        bounds: (usize, usize),
    ) -> Result<Option<bool>> {
        let mut button = None;
        let max = (
            bounds.0.saturating_sub(1) as i64,
            bounds.1.saturating_sub(1) as i64,
        );

        if let Some(pointer) = &mut self.relative {
            // The movement is in counts, and the device reports how many are in a millimeter.
            let mode = *pointer.mode();
            if let Some(state) = pointer.read_state().context("unable to read pointer")? {
                let step = |movement: i32, resolution: u64| {
                    movement as i64 * PIXELS_PER_MILLIMETER / resolution.max(1) as i64
                };
                let x = cursor.0 as i64 + step(state.relative_movement_x, mode.resolution_x);
                let y = cursor.1 as i64 + step(state.relative_movement_y, mode.resolution_y);
                *cursor = (x.clamp(0, max.0) as usize, y.clamp(0, max.1) as usize);
                button = Some(state.left_button.into());
            }
        }

        if let Some(pointer) = &mut self.absolute {
            // The position is somewhere between the minimum and maximum of the device.
            let mode = *pointer.mode();
            if let Some(state) = pointer.read_state().context("unable to read pointer")? {
                let scale = |value: u64, min: u64, device_max: u64, bound: i64| {
                    let span = device_max.saturating_sub(min).max(1) as i128;
                    (value.saturating_sub(min) as i128 * bound as i128 / span).min(bound as i128)
                        as usize
                };
                *cursor = (
                    scale(
                        state.current_x,
                        mode.absolute_min_x,
                        mode.absolute_max_x,
                        max.0,
                    ),
                    scale(
                        state.current_y,
                        mode.absolute_min_y,
                        mode.absolute_max_y,
                        max.1,
                    ),
                );
                button = Some(state.active_buttons & 1 != 0);
            }
        }

        Ok(button)
    }
}

/// The position and size of the entries on the framebuffer.
struct Layout {
    /// How many pixels each pixel of the font is drawn as.
    scale: usize,
    /// The size of a character in pixels.
    cell: usize,
    /// The column that entries start at.
    x: usize,
    /// The row that the first entry starts at.
    y: usize,
    /// The width of each entry.
    width: usize,
    /// The height of each entry.
    height: usize,
    /// The space between entries.
    gap: usize,
    /// The number of entries that fit on the framebuffer.
    visible: usize,
}

impl Layout {
    /// Calculate the layout of `entries` on a framebuffer of `screen` width and height.
    fn new(screen: (usize, usize), entries: &[BootableEntry]) -> Self {
        // Large screens get larger text so that it stays readable.
        let scale = (screen.1 / 360).clamp(1, 4);
        let cell = GLYPH_SIZE * scale;
        let (height, gap) = (cell * 2, cell / 2);

        // All entries are the width of the longest title, with room for the heart and padding.
        let longest = entries
            .iter()
            .map(|entry| entry.title().chars().count())
            .max()
            .unwrap_or(0);
        let width = ((longest + 4) * cell).min(screen.0.saturating_sub(cell * 2));

        // The title is above the entries, and the status line is below.
        let top = cell * 4;
        let space = screen.1.saturating_sub(top + cell * 3);
        let visible = entries.len().min(((space + gap) / (height + gap)).max(1));
        let list = visible * (height + gap) - gap;

        Self {
            scale,
            cell,
            x: (screen.0 - width) / 2,
            y: top + space.saturating_sub(list) / 2,
            width,
            height,
            gap,
            visible,
        }
    }

    /// The row that the entry at `slot` on the screen starts at.
    fn row(&self, slot: usize) -> usize {
        self.y + slot * (self.height + self.gap)
    }

    /// The slot of the entry that is at `position`, if there is one.
    fn slot_at(&self, position: (usize, usize)) -> Option<usize> {
        let (x, y) = (
            position.0.checked_sub(self.x)?,
            position.1.checked_sub(self.y)?,
        );
        let slot = y / (self.height + self.gap);
        (x < self.width && y % (self.height + self.gap) < self.height && slot < self.visible)
            .then_some(slot)
    }
}

/// A display that the menu is drawn to.
struct Screen {
    /// The graphics output of the display.
    gop: ScopedProtocol<GraphicsOutput>,
    /// What is drawn before it is blitted to the display.
    fb: Framebuffer,
    /// Where the entries are on this display.
    layout: Layout,
    /// The index of the first entry that is on this display.
    offset: usize,
}

impl Screen {
    /// Open every display that the firmware has, or fail if it has none.
    /// The console splitter has a graphics output that draws to every display. It has no device
    /// path, so it is only used if there are no outputs for the displays themselves.
    fn open_all(entries: &[BootableEntry]) -> Result<Vec<Self>> {
        let handles = uefi::boot::find_handles::<GraphicsOutput>()
            .context("unable to find a graphics output")?;
        let has_device_path = |handle: &Handle| {
            uefi::boot::test_protocol::<DevicePath>(OpenProtocolParams {
                handle: *handle,
                agent: uefi::boot::image_handle(),
                controller: None,
            })
            .unwrap_or(false)
        };
        let displays = handles
            .iter()
            .copied()
            .filter(has_device_path)
            .collect::<Vec<_>>();
        let handles = if displays.is_empty() {
            handles
        } else {
            displays
        };

        let mut screens = Vec::new();
        for handle in handles {
            // Opening it exclusively would disconnect the firmware's text console from the
            // display, and anything printed after the menu may not show up.
            let Ok(gop) = open_shared::<GraphicsOutput>(handle) else {
                continue;
            };
            let (width, height) = gop.current_mode_info().resolution();
            let Ok(fb) = Framebuffer::new(width, height) else {
                continue;
            };
            let layout = Layout::new((width, height), entries);
            screens.push(Self {
                gop,
                fb,
                layout,
                offset: 0,
            });
        }

        if screens.is_empty() {
            bail!("unable to open a graphics output");
        }
        Ok(screens)
    }

    /// The size of the display in pixels.
    fn size(&self) -> (usize, usize) {
        (self.fb.width(), self.fb.height())
    }

    /// Where `position` on a display of size `from` is on this display, keeping its place
    /// relative to the edges.
    fn locate(&self, position: (usize, usize), from: (usize, usize)) -> (usize, usize) {
        let scale = |value: usize, to: usize, from: usize| {
            (value * to / from.max(1)).min(to.saturating_sub(1))
        };
        (
            scale(position.0, self.fb.width(), from.0),
            scale(position.1, self.fb.height(), from.1),
        )
    }

    /// Scroll so that the `selected` entry is visible.
    fn scroll_to(&mut self, selected: usize) {
        if selected < self.offset {
            self.offset = selected;
        } else if selected >= self.offset + self.layout.visible {
            self.offset = selected + 1 - self.layout.visible;
        }
    }
}

/// What is shown on the framebuffer.
struct Scene<'a> {
    /// The entries that can be chosen.
    entries: &'a [BootableEntry],
    /// The index of the selected entry.
    selected: usize,
    /// The status line at the bottom of the screen.
    status: String,
    /// The number of ticks since the menu was shown, which moves the logo.
    ticks: usize,
}

/// Fill `rows` with a gradient from `top` to `bottom`.
fn draw_background(fb: &mut Framebuffer, top: (u8, u8, u8), bottom: (u8, u8, u8)) {
    let (width, height) = (fb.width(), fb.height());
    let mix = |from: u8, to: u8, row: usize| {
        (from as i64 + (to as i64 - from as i64) * row as i64 / height.max(1) as i64) as u8
    };
    for row in 0..height {
        let color = BltPixel::new(
            mix(top.0, bottom.0, row),
            mix(top.1, bottom.1, row),
            mix(top.2, bottom.2, row),
        );
        fb.fill_rect(0, row, width, 1, color);
    }
}

/// Fill a rectangle with the shape of a pill, which has fully rounded ends.
fn draw_pill(
    fb: &mut Framebuffer,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    color: BltPixel,
) {
    let radius = height / 2;
    for row in 0..height {
        // How far the row is from the edge, where the ends curve in.
        let edge = row.min(height - 1 - row);
        let inset = if edge >= radius {
            0
        } else {
            let rise = radius - edge;
            (0..radius)
                .find(|inset| (radius - inset).pow(2) + rise.pow(2) <= radius.pow(2))
                .unwrap_or(radius)
        };
        fb.fill_rect(
            x + inset,
            y + row,
            width.saturating_sub(inset * 2),
            1,
            color,
        );
    }
}

/// Draw `glyph` at `x` and `y`, with each pixel of it drawn as `scale` by `scale` pixels.
fn draw_glyph(
    fb: &mut Framebuffer,
    x: usize,
    y: usize,
    scale: usize,
    glyph: &Glyph,
    color: BltPixel,
) {
    for (row, bits) in glyph.iter().enumerate() {
        for column in (0..GLYPH_SIZE).filter(|column| bits >> column & 1 != 0) {
            fb.fill_rect(x + column * scale, y + row * scale, scale, scale, color);
        }
    }
}

/// Draw `text` starting at `x` and `y`, cut off to at most `columns` characters.
fn draw_text(
    fb: &mut Framebuffer,
    x: usize,
    y: usize,
    scale: usize,
    columns: usize,
    text: &str,
    color: BltPixel,
) {
    for (index, c) in text.chars().take(columns).enumerate() {
        draw_glyph(
            fb,
            x + index * GLYPH_SIZE * scale,
            y,
            scale,
            font::glyph(c),
            color,
        );
    }
}

/// Draw `text` in the middle of the framebuffer at `y`.
fn draw_centered(fb: &mut Framebuffer, y: usize, scale: usize, text: &str, color: BltPixel) {
    let columns = fb.width() / (GLYPH_SIZE * scale);
    let width = text.chars().count().min(columns) * GLYPH_SIZE * scale;
    draw_text(fb, (fb.width() - width) / 2, y, scale, columns, text, color);
}

/// Draw the logo in the bottom right corner, bouncing a little with each of `ticks`.
/// Nothing is drawn if the logo would be in the way of the entries.
fn draw_logo(fb: &mut Framebuffer, layout: &Layout, ticks: usize) {
    let factor = layout.scale.div_ceil(2);
    let (width, height) = (LOGO_WIDTH * factor, LOGO_HEIGHT * factor);
    let (Some(x), Some(floor)) = (
        fb.width().checked_sub(width + layout.cell),
        fb.height().checked_sub(height + layout.cell),
    ) else {
        return;
    };

    // The entries are in the way if the logo reaches over them, even while it is bouncing up.
    let reach = layout.row(layout.visible);
    if x < layout.x + layout.width && floor.saturating_sub(layout.cell) < reach {
        return;
    }

    // The logo rises and falls by up to four logo pixels.
    let phase = ticks % BOUNCE_TICKS;
    let y = floor - phase.min(BOUNCE_TICKS - phase) / 2 * factor;

    for row in 0..height {
        for column in 0..width {
            let source = ((row / factor) * LOGO_WIDTH + column / factor) * 4;
            let [red, green, blue, alpha] = LOGO[source..source + 4] else {
                continue;
            };
            // The channels are premultiplied, so the color is the logo plus what shows through.
            if let Some(pixel) = fb.pixel(x + column, y + row) {
                let through = 255 - alpha as u32;
                let blend =
                    |logo: u8, behind: u8| (logo as u32 + behind as u32 * through / 255) as u8;
                *pixel = BltPixel::new(
                    blend(red, pixel.red),
                    blend(green, pixel.green),
                    blend(blue, pixel.blue),
                );
            }
        }
    }
}

/// Draw the cursor with its tip at `position`.
fn draw_cursor(fb: &mut Framebuffer, scale: usize, position: (usize, usize)) {
    let scale = scale.div_ceil(2);
    for (row, line) in CURSOR.iter().enumerate() {
        for (column, shape) in line.bytes().enumerate() {
            let color = match shape {
                b'X' => CURSOR_OUTLINE,
                b'.' => CURSOR_FILL,
                _ => continue,
            };
            fb.fill_rect(
                position.0 + column * scale,
                position.1 + row * scale,
                scale,
                scale,
                color,
            );
        }
    }
}

/// Draw the whole menu to the framebuffer.
fn render(
    fb: &mut Framebuffer,
    layout: &Layout,
    scene: &Scene,
    offset: usize,
    cursor: Option<(usize, usize)>,
) {
    draw_background(fb, BACKGROUND_TOP, BACKGROUND_BOTTOM);
    draw_centered(fb, layout.cell * 2, layout.scale * 2, TITLE, ACCENT);

    for (slot, (index, entry)) in scene
        .entries
        .iter()
        .enumerate()
        .skip(offset)
        .take(layout.visible)
        .enumerate()
    {
        let selected = index == scene.selected;
        let row = layout.row(slot);
        let (pill, text) = if selected {
            (PILL_SELECTED, TEXT_SELECTED)
        } else {
            (PILL, TEXT)
        };
        draw_pill(fb, layout.x, row, layout.width, layout.height, pill);

        let text_row = row + (layout.height - layout.cell) / 2;
        if selected {
            draw_glyph(
                fb,
                layout.x + layout.cell,
                text_row,
                layout.scale,
                &HEART,
                text,
            );
        }
        draw_text(
            fb,
            layout.x + layout.cell * 5 / 2 + layout.cell / 2,
            text_row,
            layout.scale,
            (layout.width / layout.cell).saturating_sub(4),
            entry.title(),
            text,
        );
    }

    let status_row = fb.height().saturating_sub(layout.cell * 2);
    draw_centered(fb, status_row, layout.scale, &scene.status, TEXT_MUTED);
    draw_logo(fb, layout, scene.ticks);
    if let Some(position) = cursor {
        draw_cursor(fb, layout.scale, position);
    }
}

/// Run the menu until an entry is chosen, returning the index of that entry.
/// The `selected` entry is booted if there is no input before `timeout` passes.
/// Without a timeout, the menu waits for the user.
fn run(
    input: &mut Input,
    mut mouse: Option<Mouse>,
    screens: &mut [Screen],
    timeout: Option<Duration>,
    entries: &[BootableEntry],
    selected: usize,
) -> Result<usize> {
    // The cursor moves within the first screen, and is shown at the same place on the others.
    let bounds = screens[0].size();
    let page = screens[0].layout.visible;
    let last = entries.len() - 1;

    // The events that wake the menu up, which are keys and pointer input.
    let mut events = Vec::from([input
        .wait_for_key_event()
        .context("unable to acquire key event")?]);
    if let Some(mouse) = &mouse {
        events.extend(mouse.events()?);
    }

    let mut scene = Scene {
        entries,
        selected,
        status: String::new(),
        ticks: 0,
    };
    let mut cursor = mouse.as_ref().map(|_| (bounds.0 / 2, bounds.1 / 2));
    // The time left before booting, or None once input stops the countdown.
    let mut remaining = timeout;
    let mut button_down = false;

    loop {
        scene.status = match remaining {
            Some(remaining) => format!("Boot in {} s.", remaining.as_millis().div_ceil(1000)),
            None => String::from(HINT),
        };
        for screen in screens.iter_mut() {
            screen.scroll_to(scene.selected);
            let cursor = cursor.map(|position| screen.locate(position, bounds));
            render(
                &mut screen.fb,
                &screen.layout,
                &scene,
                screen.offset,
                cursor,
            );
            screen.fb.blit(&mut screen.gop)?;
        }

        // Wake up every tick to move the logo and update the countdown.
        if wait_for_events(&events, Some(TICK))?.is_none() {
            scene.ticks += 1;
            if let Some(left) = remaining {
                let left = left.saturating_sub(TICK);
                if left.is_zero() {
                    return Ok(scene.selected);
                }
                remaining = Some(left);
            }
            continue;
        }

        // Some firmware signals an event without any input being available.
        if let Some(key) = input.read_key().context("unable to read key")? {
            remaining = None;
            match key {
                Key::Special(ScanCode::UP) => scene.selected = scene.selected.saturating_sub(1),
                Key::Special(ScanCode::DOWN) => scene.selected = (scene.selected + 1).min(last),
                Key::Special(ScanCode::HOME) => scene.selected = 0,
                Key::Special(ScanCode::END) => scene.selected = last,
                Key::Special(ScanCode::PAGE_UP) => {
                    scene.selected = scene.selected.saturating_sub(page)
                }
                Key::Special(ScanCode::PAGE_DOWN) => {
                    scene.selected = (scene.selected + page).min(last)
                }

                // Serial consoles may send a line feed instead of a carriage return.
                Key::Printable(c) if matches!(char::from(c), '\r' | '\n') => {
                    return Ok(scene.selected);
                }

                _ => {}
            }
        }

        if let (Some(mouse), Some(cursor)) = (mouse.as_mut(), cursor.as_mut())
            && let Some(down) = mouse.poll(cursor, bounds)?
        {
            remaining = None;

            // Hovering over an entry selects it, and pressing the button boots it.
            let hovered = screens.iter().find_map(|screen| {
                let position = screen.locate(*cursor, bounds);
                screen
                    .layout
                    .slot_at(position)
                    .map(|slot| screen.offset + slot)
            });
            if let Some(entry) = hovered {
                scene.selected = entry;
                if down && !button_down {
                    return Ok(scene.selected);
                }
            }
            button_down = down;
        }
    }
}

impl BootMenu for GraphicalMenu {
    /// Selects an entry using the graphical menu.
    /// The menu is drawn to every display, and fails if the firmware has none.
    fn select<'a>(
        &self,
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

        let mut screens = Screen::open_all(entries)?;
        let mut mouse = Mouse::open();

        let result = uefi::system::with_stdin(|input| {
            run(input, mouse.take(), &mut screens, timeout, entries, default)
        });

        // Clear the console so that anything printed after the menu is readable.
        let _ = uefi::system::with_stdout(|output| output.clear());

        Ok(&entries[result?])
    }
}

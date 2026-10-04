use crate::entries::BootableEntry;
use crate::menu::font::{self, ELLIPSIS, GLYPH_SIZE, Glyph, HEART};
use crate::menu::logo::{LOGO, LOGO_HEIGHT, LOGO_WIDTH};
use crate::menu::{BootMenu, wait_for_events};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use anyhow::{Context, Result, bail};
use core::time::Duration;
use eficore::framebuffer::Framebuffer;
use uefi::boot::{OpenProtocolParams, ScopedProtocol};
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
const HINT: &str = "Pick an entry with the arrow keys.";

/// The hint shown instead of [HINT] when the mouse is in use.
const HINT_MOUSE: &str = "Pick an entry with the arrow keys or the mouse.";

/// The color of the top of the background, which fades into [BACKGROUND_BOTTOM].
const BACKGROUND_TOP: (u8, u8, u8) = (0x3a, 0x1c, 0x4e);

/// The color of the bottom of the background.
const BACKGROUND_BOTTOM: (u8, u8, u8) = (0x16, 0x0c, 0x24);

/// The widest that the status line gets, which the panel has room for.
const STATUS_WIDEST: &str = "Boot in 999 s.";

/// The color of the panel behind the entries.
const PANEL: BltPixel = BltPixel::new(0x22, 0x12, 0x34);

/// The color of the edge of the panel, the line below the entries and the scrollbar track.
const PANEL_EDGE: BltPixel = BltPixel::new(0x4a, 0x2e, 0x66);

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

/// A graphical boot menu that selects entries with the keyboard, or the mouse if enabled.
pub struct GraphicalMenu {
    /// Whether the mouse is used when the firmware has a pointing device.
    pub enable_mouse: bool,
}

/// The pointing devices of the firmware.
/// Firmware commonly has both kinds of protocol, even if one has no device behind it.
struct Mouse {
    /// A device that reports how far it has moved, like a mouse.
    relative: Option<ScopedProtocol<Pointer>>,
    /// A device that reports where it is, like a touch screen or the tablet of a virtual machine.
    absolute: Option<ScopedProtocol<AbsolutePointer>>,
}

/// Open the pointer protocol `P`, preferring the one on the console input handle.
/// That one belongs to the console splitter, which combines every device and is never removed.
/// Otherwise, the first device that has the protocol is used.
fn open_pointer<P: ProtocolPointer + ?Sized>() -> Option<ScopedProtocol<P>> {
    // SAFETY: The handle in the system table is either null or a valid handle.
    let console = uefi::table::system_table_raw()
        .and_then(|table| unsafe { Handle::from_ptr(table.as_ref().stdin_handle) });
    console
        .and_then(|handle| eficore::handle::open_shared::<P>(handle).ok())
        .or_else(|| {
            uefi::boot::get_handle_for_protocol::<P>()
                .and_then(eficore::handle::open_shared::<P>)
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

/// The position and size of everything on the framebuffer.
/// The logo and the name are in a header, with a panel below it that holds the entries.
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
    /// The column, row, width and height of the panel behind the entries.
    panel: (usize, usize, usize, usize),
    /// How round the corners of the panel are.
    radius: usize,
    /// The column that the text of an entry starts at.
    text_x: usize,
    /// How many characters of a title fit in an entry.
    columns: usize,
    /// The row of the line between the entries and the status line.
    footer: usize,
    /// The column, row, width and height of the logo when it is on the ground.
    logo: (usize, usize, usize, usize),
    /// How many pixels the logo rises while it bounces.
    bounce: usize,
    /// The column, row and scale of the name, if there is room for it.
    title: Option<(usize, usize, usize)>,
    /// Whether there are more entries than fit, which adds a scrollbar.
    scrolls: bool,
}

impl Layout {
    /// Calculate the layout of `entries` on a framebuffer of `screen` width and height.
    fn new(screen: (usize, usize), entries: &[BootableEntry]) -> Self {
        let (width, height) = screen;

        // Large screens get larger text so that it stays readable, but not so large that the
        // titles take up the whole screen.
        let scale = (height / 540).min(width / 640).clamp(1, 3);
        let cell = GLYPH_SIZE * scale;
        let margin = (cell * 2).max(height / 20).min(width / 8);
        let padding = cell * 3 / 2;
        let (row, gap) = (cell * 2, scale * 2);
        let header_gap = cell * 2;

        // The line between the entries and the status, and the status below it.
        let footer_height = cell + scale + cell / 2 + cell;

        // The panel is the width of the longest title, but never most of the screen.
        let longest = entries
            .iter()
            .map(|entry| entry.title().chars().count())
            .max()
            .unwrap_or(0);
        let status = HINT.len().max(HINT_MOUSE.len()).max(STATUS_WIDEST.len());
        let panel_width = ((longest + 4) * cell + padding * 2)
            .max(40 * cell)
            .max((status + 8) * cell)
            .min(width.saturating_sub(margin * 2).min(width * 3 / 5));

        // The header is as tall as the logo, and the logo needs room to bounce.
        let fit = |logo: usize| {
            let header = logo + logo / 16;
            let space = height
                .saturating_sub(margin * 2 + header + header_gap + padding * 2 + footer_height);
            let visible = ((space + gap) / (row + gap)).clamp(1, entries.len().max(1));
            (header, visible)
        };
        let mut logo_height = (LOGO_HEIGHT * 2).min(height / 7).max(32);
        let (mut header, mut visible) = fit(logo_height);
        // A short screen gets a smaller logo before it loses entries.
        if visible < entries.len().min(3) {
            logo_height = (logo_height / 2).max(32);
            (header, visible) = fit(logo_height);
        }
        let logo_width = logo_height * LOGO_WIDTH / LOGO_HEIGHT;
        let bounce = logo_height / 16;

        // The name is next to the logo, if the two fit together.
        let title_scale = (logo_height / 32).clamp(scale, scale * 3);
        let title_width = TITLE.len() * GLYPH_SIZE * title_scale;
        let fits = logo_width + cell + title_width <= width.saturating_sub(margin * 2);
        let group = if fits {
            logo_width + cell + title_width
        } else {
            logo_width
        };
        let group_x = width.saturating_sub(group) / 2;

        let list = visible * (row + gap) - gap;
        let panel_height = padding * 2 + list + footer_height;
        let top = (height.saturating_sub(header + header_gap + panel_height) / 2).max(margin);
        let panel_x = width.saturating_sub(panel_width) / 2;
        let panel_y = top + header + header_gap;

        // The scrollbar is in a column next to the entries.
        let scrolls = visible < entries.len();
        let gutter = if scrolls { cell } else { 0 };
        let list_width = panel_width.saturating_sub(padding * 2 + gutter);
        let list_y = panel_y + padding;

        Self {
            scale,
            cell,
            x: panel_x + padding,
            y: list_y,
            width: list_width,
            height: row,
            gap,
            visible,
            panel: (panel_x, panel_y, panel_width, panel_height),
            radius: cell,
            text_x: panel_x + padding + cell * 3,
            columns: list_width.saturating_sub(cell * 4) / cell,
            footer: list_y + list + cell,
            logo: (group_x, top + bounce, logo_width, logo_height),
            bounce,
            title: fits.then_some((
                group_x + logo_width + cell,
                top + bounce + logo_height.saturating_sub(GLYPH_SIZE * title_scale) / 2,
                title_scale,
            )),
            scrolls,
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
    /// What was last blitted to the display, which only has to be updated where it differs.
    shown: Framebuffer,
    /// The parts of the menu that never change, which each frame starts from.
    base: Framebuffer,
    /// Whether the display has had a frame blitted to it.
    drawn: bool,
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
            let Ok(gop) = eficore::handle::open_shared::<GraphicsOutput>(handle) else {
                continue;
            };
            let (width, height) = gop.current_mode_info().resolution();
            let (Ok(fb), Ok(shown), Ok(mut base)) = (
                Framebuffer::new(width, height),
                Framebuffer::new(width, height),
                Framebuffer::new(width, height),
            ) else {
                continue;
            };
            let layout = Layout::new((width, height), entries);
            render_base(&mut base, &layout);
            screens.push(Self {
                gop,
                fb,
                shown,
                base,
                drawn: false,
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

/// Fill a rectangle that has rounded corners of `radius`.
fn draw_rounded(
    fb: &mut Framebuffer,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    radius: usize,
    color: BltPixel,
) {
    if width == 0 || height == 0 {
        return;
    }
    let radius = radius.min(height / 2).min(width / 2);
    for row in 0..height {
        // How far the row is from the edge, where the corners curve in.
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

/// Fill a rectangle with the shape of a pill, which has fully rounded ends.
fn draw_pill(
    fb: &mut Framebuffer,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    color: BltPixel,
) {
    draw_rounded(fb, x, y, width, height, height / 2, color);
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

/// Draw `text` starting at `x` and `y`, cut off with an ellipsis if it is over `columns` characters.
fn draw_text(
    fb: &mut Framebuffer,
    x: usize,
    y: usize,
    scale: usize,
    columns: usize,
    text: &str,
    color: BltPixel,
) {
    let cut = text.chars().count() > columns;
    let shown = if cut {
        columns.saturating_sub(1)
    } else {
        columns
    };
    for (index, c) in text.chars().take(shown).enumerate() {
        draw_glyph(
            fb,
            x + index * GLYPH_SIZE * scale,
            y,
            scale,
            font::glyph(c),
            color,
        );
    }
    if cut && columns > 0 {
        draw_glyph(
            fb,
            x + shown * GLYPH_SIZE * scale,
            y,
            scale,
            &ELLIPSIS,
            color,
        );
    }
}

/// Draw `text` in the middle of `x` and `width` at `y`.
fn draw_centered(
    fb: &mut Framebuffer,
    x: usize,
    width: usize,
    y: usize,
    scale: usize,
    text: &str,
    color: BltPixel,
) {
    let columns = width / (GLYPH_SIZE * scale);
    let used = text.chars().count().min(columns) * GLYPH_SIZE * scale;
    draw_text(fb, x + (width - used) / 2, y, scale, columns, text, color);
}

/// Draw the logo above the entries, bouncing a little with each of `ticks`.
fn draw_logo(fb: &mut Framebuffer, layout: &Layout, ticks: usize) {
    let (x, floor, width, height) = layout.logo;

    // The logo rises and falls by up to `bounce` pixels.
    let phase = ticks % BOUNCE_TICKS;
    let rise = phase.min(BOUNCE_TICKS - phase) * layout.bounce / (BOUNCE_TICKS / 2);
    let y = floor - rise;

    for row in 0..height {
        let source_row = row * LOGO_HEIGHT / height * LOGO_WIDTH;
        for column in 0..width {
            let source = (source_row + column * LOGO_WIDTH / width) * 4;
            let [red, green, blue, alpha] = LOGO[source..source + 4] else {
                continue;
            };
            if alpha == 0 {
                continue;
            }
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

/// Draw the track and the thumb of the scrollbar, which shows where `offset` is in `total`.
fn draw_scrollbar(fb: &mut Framebuffer, layout: &Layout, offset: usize, total: usize) {
    let height = layout.visible * (layout.height + layout.gap) - layout.gap;
    let x = layout.x + layout.width + layout.cell / 2 - layout.scale;
    let width = layout.scale * 2;
    fb.fill_rect(x, layout.y, width, height, PANEL_EDGE);

    let thumb = (height * layout.visible / total)
        .max(layout.cell)
        .min(height);
    let travel = total.saturating_sub(layout.visible).max(1);
    let y = layout.y + (height - thumb) * offset.min(travel) / travel;
    draw_rounded(fb, x, y, width, thumb, width / 2, ACCENT);
}

/// Draw the cursor with its tip at `position`.
fn draw_cursor(fb: &mut Framebuffer, scale: usize, position: (usize, usize)) {
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

/// Draw the parts of the menu that never change to the framebuffer.
fn render_base(fb: &mut Framebuffer, layout: &Layout) {
    draw_background(fb, BACKGROUND_TOP, BACKGROUND_BOTTOM);
    if let Some((x, y, scale)) = layout.title {
        draw_text(fb, x, y, scale, TITLE.len(), TITLE, ACCENT);
    }

    let (panel_x, panel_y, panel_width, panel_height) = layout.panel;
    let edge = layout.scale;
    draw_rounded(
        fb,
        panel_x,
        panel_y,
        panel_width,
        panel_height,
        layout.radius,
        PANEL_EDGE,
    );
    draw_rounded(
        fb,
        panel_x + edge,
        panel_y + edge,
        panel_width.saturating_sub(edge * 2),
        panel_height.saturating_sub(edge * 2),
        layout.radius.saturating_sub(edge),
        PANEL,
    );

    // The line between the entries and the status line.
    let inner = panel_width.saturating_sub(layout.cell * 3);
    let inner_x = panel_x + layout.cell * 3 / 2;
    fb.fill_rect(inner_x, layout.footer, inner, layout.scale, PANEL_EDGE);
}

/// Draw the whole menu to the framebuffer, which already has the [render_base] drawn.
fn render(
    fb: &mut Framebuffer,
    layout: &Layout,
    scene: &Scene,
    offset: usize,
    cursor: Option<(usize, usize)>,
) {
    draw_logo(fb, layout, scene.ticks);

    let (panel_x, _, panel_width, _) = layout.panel;
    for (slot, (index, entry)) in scene
        .entries
        .iter()
        .enumerate()
        .skip(offset)
        .take(layout.visible)
        .enumerate()
    {
        let row = layout.row(slot);
        let text_row = row + (layout.height - layout.cell) / 2;
        // Only the selected entry has a pill, which keeps the list calm.
        let text = if index == scene.selected {
            draw_pill(
                fb,
                layout.x,
                row,
                layout.width,
                layout.height,
                PILL_SELECTED,
            );
            draw_glyph(
                fb,
                layout.x + layout.cell,
                text_row,
                layout.scale,
                &HEART,
                TEXT_SELECTED,
            );
            TEXT_SELECTED
        } else {
            TEXT
        };
        draw_text(
            fb,
            layout.text_x,
            text_row,
            layout.scale,
            layout.columns,
            entry.title(),
            text,
        );
    }

    if layout.scrolls {
        draw_scrollbar(fb, layout, offset, scene.entries.len());
    }

    // The status line is below the line across the panel, with the position when it scrolls.
    let inner = panel_width.saturating_sub(layout.cell * 3);
    let inner_x = panel_x + layout.cell * 3 / 2;
    let status_row = layout.footer + layout.scale + layout.cell / 2;
    // The position is at the right edge, so the status keeps clear of it on both sides.
    let keep_clear = if layout.scrolls { layout.cell * 8 } else { 0 };
    draw_centered(
        fb,
        inner_x + keep_clear,
        inner.saturating_sub(keep_clear * 2),
        status_row,
        layout.scale,
        &scene.status,
        TEXT_MUTED,
    );
    if layout.scrolls {
        let position = format!("{}/{}", scene.selected + 1, scene.entries.len());
        let width = position.len() * layout.cell;
        draw_text(
            fb,
            (inner_x + inner).saturating_sub(width),
            status_row,
            layout.scale,
            position.len(),
            &position,
            TEXT_MUTED,
        );
    }

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
            None if mouse.is_some() => String::from(HINT_MOUSE),
            None => String::from(HINT),
        };
        for screen in screens.iter_mut() {
            screen.scroll_to(scene.selected);
            let cursor = cursor.map(|position| screen.locate(position, bounds));
            screen.fb.copy_from(&screen.base);
            render(
                &mut screen.fb,
                &screen.layout,
                &scene,
                screen.offset,
                cursor,
            );
            // Only the first frame has to fill the display, the rest only change parts of it.
            if screen.drawn {
                screen.fb.blit_changes(&screen.shown, &mut screen.gop)?;
            } else {
                screen.fb.blit(&mut screen.gop)?;
                screen.drawn = true;
            }
            core::mem::swap(&mut screen.fb, &mut screen.shown);
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
        let mut mouse = self.enable_mouse.then(Mouse::open).flatten();

        let result = uefi::system::with_stdin(|input| {
            run(input, mouse.take(), &mut screens, timeout, entries, default)
        });

        // Clear the console so that anything printed after the menu is readable.
        let _ = uefi::system::with_stdout(|output| output.clear());

        Ok(&entries[result?])
    }
}

use alloc::vec;
use alloc::vec::Vec;
use anyhow::{Context, Result, bail};
use uefi::proto::console::gop::{BltOp, BltPixel, BltRegion, GraphicsOutput};

/// How many unchanged rows may sit between changed rows before they are blitted separately.
const BAND_GAP: usize = 8;

/// Represents the EFI framebuffer.
pub struct Framebuffer {
    /// The width of the framebuffer in pixels.
    width: usize,
    /// The height of the framebuffer in pixels.
    height: usize,
    /// The pixels of the framebuffer.
    pixels: Vec<BltPixel>,
}

impl Framebuffer {
    /// Creates a new framebuffer of the specified `width` and `height`.
    pub fn new(width: usize, height: usize) -> Result<Self> {
        // Verify that the size is valid during multiplication.
        let size = width
            .checked_mul(height)
            .context("framebuffer size overflow")?;

        // Initialize the pixel buffer with black pixels, with the verified size.
        let pixels = vec![BltPixel::new(0, 0, 0); size];

        Ok(Framebuffer {
            width,
            height,
            pixels,
        })
    }

    /// The width of the framebuffer in pixels.
    pub fn width(&self) -> usize {
        self.width
    }

    /// The height of the framebuffer in pixels.
    pub fn height(&self) -> usize {
        self.height
    }

    /// Fill the rectangle at `x` and `y` of `width` and `height` with `color`.
    /// The parts of the rectangle outside of the framebuffer are not drawn.
    pub fn fill_rect(&mut self, x: usize, y: usize, width: usize, height: usize, color: BltPixel) {
        let end_x = x.saturating_add(width).min(self.width);
        let end_y = y.saturating_add(height).min(self.height);
        if x >= end_x {
            return;
        }
        for row in y..end_y {
            // The row and columns are in bounds, so its range is within the pixels.
            self.pixels[row * self.width + x..row * self.width + end_x].fill(color);
        }
    }

    /// Mutably acquires a pixel of the framebuffer at the specified `x` and `y` coordinate.
    pub fn pixel(&mut self, x: usize, y: usize) -> Option<&mut BltPixel> {
        // Verify that the coordinates are within the bounds of the framebuffer.
        if x >= self.width || y >= self.height {
            return None;
        }

        // Calculate the index of the pixel safely, returning None if it overflows.
        let index = y.checked_mul(self.width)?.checked_add(x)?;
        // Return the pixel at the index. If the index is out of bounds, this will return None.
        self.pixels.get_mut(index)
    }

    /// Replace the pixels of the framebuffer with those of `other`, which must be the same size.
    pub fn copy_from(&mut self, other: &Framebuffer) {
        self.pixels.copy_from_slice(&other.pixels);
    }

    /// Blit the framebuffer to the specified `gop` [GraphicsOutput].
    pub fn blit(&self, gop: &mut GraphicsOutput) -> Result<()> {
        gop.blt(BltOp::BufferToVideo {
            buffer: &self.pixels,
            src: BltRegion::Full,
            dest: (0, 0),
            dims: (self.width, self.height),
        })
        .context("unable to blit framebuffer")?;
        Ok(())
    }

    /// Blit only what is different from `previous`, which must be the same size, to the specified
    /// `gop` [GraphicsOutput]. Changes that are far apart vertically are blitted separately, so
    /// that the pixels between them are not sent to the display.
    pub fn blit_changes(&self, previous: &Framebuffer, gop: &mut GraphicsOutput) -> Result<()> {
        if self.width != previous.width || self.height != previous.height {
            bail!("framebuffer sizes differ");
        }

        // The top, bottom, left and right of the changed rows that haven't been blitted yet.
        let mut band: Option<(usize, usize, usize, usize)> = None;
        let mut quiet = 0;
        for row in 0..self.height {
            let range = row * self.width..(row + 1) * self.width;
            let (now, before) = (&self.pixels[range.clone()], &previous.pixels[range]);
            let differs = |(now, before): (&BltPixel, &BltPixel)| {
                (now.red, now.green, now.blue) != (before.red, before.green, before.blue)
            };
            let first = now.iter().zip(before).position(differs);
            let Some(first) = first else {
                quiet += 1;
                if quiet > BAND_GAP
                    && let Some(band) = band.take()
                {
                    self.blit_region(gop, band)?;
                }
                continue;
            };
            let last = now.iter().zip(before).rposition(differs).unwrap_or(first);
            quiet = 0;
            band = Some(match band {
                Some((top, _, left, right)) => (top, row + 1, left.min(first), right.max(last + 1)),
                None => (row, row + 1, first, last + 1),
            });
        }
        if let Some(band) = band {
            self.blit_region(gop, band)?;
        }
        Ok(())
    }

    /// Blit the region of the `top`, `bottom`, `left` and `right` to the specified `gop`.
    fn blit_region(
        &self,
        gop: &mut GraphicsOutput,
        (top, bottom, left, right): (usize, usize, usize, usize),
    ) -> Result<()> {
        gop.blt(BltOp::BufferToVideo {
            buffer: &self.pixels,
            src: BltRegion::SubRectangle {
                coords: (left, top),
                px_stride: self.width,
            },
            dest: (left, top),
            dims: (right - left, bottom - top),
        })
        .context("unable to blit framebuffer region")?;
        Ok(())
    }
}

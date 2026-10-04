use alloc::vec;
use alloc::vec::Vec;
use anyhow::{Context, Result};
use uefi::proto::console::gop::{BltOp, BltPixel, BltRegion, GraphicsOutput};

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
}

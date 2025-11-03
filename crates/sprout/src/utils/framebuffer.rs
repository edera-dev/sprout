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

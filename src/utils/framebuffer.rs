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
    pub fn new(width: usize, height: usize) -> Self {
        Framebuffer {
            width,
            height,
            pixels: vec![BltPixel::new(0, 0, 0); width * height],
        }
    }

    /// Mutably acquires a pixel of the framebuffer at the specified `x` and `y` coordinate.
    pub fn pixel(&mut self, x: usize, y: usize) -> Option<&mut BltPixel> {
        self.pixels.get_mut(y * self.width + x)
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

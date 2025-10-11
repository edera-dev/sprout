use uefi::proto::console::gop::{BltOp, BltPixel, BltRegion, GraphicsOutput};

pub struct Framebuffer {
    width: usize,
    height: usize,
    pixels: Vec<BltPixel>,
}

impl Framebuffer {
    pub fn new(width: usize, height: usize) -> Self {
        Framebuffer {
            width,
            height,
            pixels: vec![BltPixel::new(0, 0, 0); width * height],
        }
    }

    pub fn pixel(&mut self, x: usize, y: usize) -> Option<&mut BltPixel> {
        self.pixels.get_mut(y * self.width + x)
    }

    pub fn blit(&self, gop: &mut GraphicsOutput) {
        gop.blt(BltOp::BufferToVideo {
            buffer: &self.pixels,
            src: BltRegion::Full,
            dest: (0, 0),
            dims: (self.width, self.height),
        })
        .expect("failed to blit framebuffer");
    }
}

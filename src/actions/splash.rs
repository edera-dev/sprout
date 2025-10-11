use crate::context::SproutContext;
use crate::utils::framebuffer::Framebuffer;
use crate::utils::read_file_contents;
use anyhow::{Context, Result};
use image::imageops::{FilterType, resize};
use image::math::Rect;
use image::{DynamicImage, ImageBuffer, ImageFormat, ImageReader, Rgba};
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::rc::Rc;
use std::time::Duration;
use uefi::boot::ScopedProtocol;
use uefi::proto::console::gop::GraphicsOutput;

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct SplashConfiguration {
    pub image: String,
    #[serde(default = "default_splash_time")]
    pub time: u32,
}

pub fn default_splash_time() -> u32 {
    5
}

fn setup_graphics() -> Result<ScopedProtocol<GraphicsOutput>> {
    let gop_handle = uefi::boot::get_handle_for_protocol::<GraphicsOutput>()
        .context("failed to get graphics output")?;
    uefi::boot::open_protocol_exclusive::<GraphicsOutput>(gop_handle)
        .context("failed to open graphics output")
}

fn fit_to_frame(image: &DynamicImage, frame: Rect) -> Rect {
    let input = Rect {
        x: 0,
        y: 0,
        width: image.width(),
        height: image.height(),
    };

    let input_ratio = input.width as f32 / input.height as f32;
    let frame_ratio = frame.width as f32 / frame.height as f32;

    let mut output = Rect {
        x: 0,
        y: 0,
        width: frame.width,
        height: frame.height,
    };

    if input_ratio < frame_ratio {
        output.width = (frame.height as f32 * input_ratio).floor() as u32;
        output.height = frame.height;
        output.x = frame.x + (frame.width - output.width) / 2;
        output.y = frame.y;
    } else {
        output.width = frame.width;
        output.height = (frame.width as f32 / input_ratio).floor() as u32;
        output.x = frame.x;
        output.y = frame.y + (frame.height - output.height) / 2;
    }

    output
}

fn resize_to_fit(image: &DynamicImage, frame: Rect) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let image = image.to_rgba8();
    resize(&image, frame.width, frame.height, FilterType::Lanczos3)
}

fn draw(image: DynamicImage) -> Result<()> {
    let mut gop = setup_graphics()?;
    let (width, height) = gop.current_mode_info().resolution();
    let display_frame = Rect {
        x: 0,
        y: 0,
        width: width as _,
        height: height as _,
    };
    let fit = fit_to_frame(&image, display_frame);
    let image = resize_to_fit(&image, fit);

    let mut framebuffer = Framebuffer::new(width, height);
    for (x, y, pixel) in image.enumerate_pixels() {
        let Some(fb) = framebuffer.pixel((x + fit.x) as usize, (fit.y + y) as usize) else {
            continue;
        };
        fb.red = pixel[0];
        fb.green = pixel[1];
        fb.blue = pixel[2];
    }

    framebuffer.blit(&mut gop)?;
    Ok(())
}

pub fn splash(context: Rc<SproutContext>, configuration: &SplashConfiguration) -> Result<()> {
    let image = context.stamp(&configuration.image);
    let image = read_file_contents(&image)?;
    let image = ImageReader::with_format(Cursor::new(image), ImageFormat::Png)
        .decode()
        .context("failed to decode splash image")?;
    draw(image)?;
    std::thread::sleep(Duration::from_secs(configuration.time as u64));
    Ok(())
}

use crate::context::SproutContext;
use crate::utils::framebuffer::Framebuffer;
use crate::utils::read_file_contents;
use anyhow::{Context, Result, bail};
use image::imageops::{FilterType, resize};
use image::math::Rect;
use image::{DynamicImage, ImageBuffer, ImageFormat, ImageReader, Rgba};
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::rc::Rc;
use std::time::Duration;
use uefi::boot::ScopedProtocol;
use uefi::proto::console::gop::GraphicsOutput;

/// We set the default splash time to zero, as this makes it so any logging shows up
/// on top of the splash and does not hold up the boot process.
const DEFAULT_SPLASH_TIME: u32 = 0;

/// The configuration of the splash action.
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct SplashConfiguration {
    /// The path to the image to display.
    /// Currently, only PNG images are supported.
    pub image: String,
    /// The time to display the splash image without interruption, in seconds.
    /// The default value is `0` which will display the image and let everything
    /// continue.
    #[serde(default = "default_splash_time")]
    pub time: u32,
}

fn default_splash_time() -> u32 {
    DEFAULT_SPLASH_TIME
}

/// Acquire the [GraphicsOutput]. We will find the first graphics output only.
fn setup_graphics() -> Result<ScopedProtocol<GraphicsOutput>> {
    // Grab the handle for the graphics output protocol.
    let gop_handle = uefi::boot::get_handle_for_protocol::<GraphicsOutput>()
        .context("unable to get graphics output")?;
    // Open the graphics output protocol exclusively.
    uefi::boot::open_protocol_exclusive::<GraphicsOutput>(gop_handle)
        .context("unable to open graphics output")
}

/// Produces a [Rect] that fits the `image` inside the specified `frame`.
/// The output [Rect] should be used to resize the image.
fn fit_to_frame(image: &DynamicImage, frame: Rect) -> Rect {
    // Convert the image dimensions to a [Rect].
    let input = Rect {
        x: 0,
        y: 0,
        width: image.width(),
        height: image.height(),
    };

    // Handle the case where the image is zero-sized.
    if input.height == 0 || input.width == 0 {
        return input;
    }

    // Calculate the ratio of the image dimensions.
    let input_ratio = input.width as f32 / input.height as f32;

    // Calculate the ratio of the frame dimensions.
    let frame_ratio = frame.width as f32 / frame.height as f32;

    // Create [Rect] to store the output dimensions.
    let mut output = Rect {
        x: 0,
        y: 0,
        width: frame.width,
        height: frame.height,
    };

    // Handle the case where the output is zero-sized.
    if output.height == 0 || output.width == 0 {
        return output;
    }

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

/// Resize the input `image` to fit the `frame`.
fn resize_to_fit(image: &DynamicImage, frame: Rect) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let image = image.to_rgba8();
    resize(&image, frame.width, frame.height, FilterType::Lanczos3)
}

/// Draw the `image` on the screen using [GraphicsOutput].
fn draw(image: DynamicImage) -> Result<()> {
    // Acquire the [GraphicsOutput] protocol.
    let mut gop = setup_graphics()?;

    // Acquire the current screen size.
    let (width, height) = gop.current_mode_info().resolution();

    // Create a display frame.
    let display_frame = Rect {
        x: 0,
        y: 0,
        width: width as _,
        height: height as _,
    };

    // Fit the image to the display frame.
    let fit = fit_to_frame(&image, display_frame);

    // If the image is zero-sized, then we should bail with an error.
    if fit.width == 0 || fit.height == 0 {
        bail!("calculated frame size is zero");
    }

    // Resize the image to fit the display frame.
    let image = resize_to_fit(&image, fit);

    // Create a framebuffer to draw the image on.
    let mut framebuffer =
        Framebuffer::new(width, height).context("unable to create framebuffer")?;

    // Iterate over the pixels in the image and put them on the framebuffer.
    for (x, y, pixel) in image.enumerate_pixels() {
        let Some(fb) = framebuffer.pixel((x + fit.x) as usize, (fit.y + y) as usize) else {
            continue;
        };
        fb.red = pixel[0];
        fb.green = pixel[1];
        fb.blue = pixel[2];
    }

    // Blit the framebuffer to the screen.
    framebuffer.blit(&mut gop)?;
    Ok(())
}

/// Runs the splash action with the specified `configuration` inside the provided `context`.
pub fn splash(context: Rc<SproutContext>, configuration: &SplashConfiguration) -> Result<()> {
    // Stamp the image path value.
    let image = context.stamp(&configuration.image);
    // Read the image contents.
    let image = read_file_contents(Some(context.root().loaded_image_path()?), &image)?;
    // Decode the image as a PNG.
    let image = ImageReader::with_format(Cursor::new(image), ImageFormat::Png)
        .decode()
        .context("unable to decode splash image")?;
    // Draw the image on the screen.
    draw(image)?;

    // Sleep for the specified time.
    std::thread::sleep(Duration::from_secs(configuration.time as u64));

    // Return control to sprout.
    Ok(())
}

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use anyhow::{Context, Result};
use uefi::proto::loaded_image::LoadedImage;

/// Loads the command-line arguments passed to the current image.
pub fn args() -> Result<Vec<String>> {
    // Acquire the current image handle.
    let handle = uefi::boot::image_handle();

    // Open the LoadedImage protocol for the current image.
    let loaded_image = uefi::boot::open_protocol_exclusive::<LoadedImage>(handle)
        .context("unable to open loaded image protocol for current image")?;

    // Load the command-line argument string.
    // Load options are usually a null-terminated UCS-2 string, but firmware is not consistent
    // about this. Some firmware passes an odd number of bytes, leaves off the null terminator,
    // or pads the string with extra nulls. We decode everything up to the first null and
    // replace anything that isn't valid UCS-2, instead of refusing to boot.
    let Some(options) = loaded_image.load_options_as_bytes() else {
        // No load options were passed. We will return an empty vector.
        return Ok(Vec::new());
    };

    // Decode the options as little-endian UTF-16, stopping at the first null.
    // A trailing odd byte can't be part of a character, so it is dropped.
    let options = options
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes(*pair))
        .take_while(|c| *c != 0)
        .collect::<Vec<u16>>();
    let options = String::from_utf16_lossy(&options);

    // Use shlex to parse the options.
    // If shlex fails, we will perform a simple whitespace split.
    let mut args = shlex::split(&options).unwrap_or_else(|| {
        options
            .split_ascii_whitespace()
            .map(|string| string.to_string())
            .collect::<Vec<_>>()
    });

    // Correct firmware that may add invalid arguments at the start.
    // Witnessed this on a Dell Precision 5690 when direct booting.
    args = args
        .into_iter()
        .skip_while(|arg| {
            arg.chars()
                .next()
                // Filter out unprintable characters and backticks.
                // Both of which have been observed in the wild.
                .map(|c| c.is_ascii_control() || c == '`')
                .unwrap_or(false)
        })
        .collect();

    // If there is a first argument, check if it is not an option.
    // If it is not, we will assume it is the path to the executable and remove it.
    if let Some(arg) = args.first()
        && !arg.starts_with('-')
    {
        args.remove(0);
    }

    Ok(args)
}

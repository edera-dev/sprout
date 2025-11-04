use alloc::string::{String, ToString};
use alloc::vec::Vec;
use anyhow::{Context, Result, bail};
use uefi::proto::loaded_image::{LoadOptionsError, LoadedImage};

/// Loads the command-line arguments passed to the current image.
pub fn args() -> Result<Vec<String>> {
    // Acquire the current image handle.
    let handle = uefi::boot::image_handle();

    // Open the LoadedImage protocol for the current image.
    let loaded_image = uefi::boot::open_protocol_exclusive::<LoadedImage>(handle)
        .context("unable to open loaded image protocol for current image")?;

    // Load the command-line argument string.
    let options = match loaded_image.load_options_as_cstr16() {
        // Load options were passed. We will return them for processing.
        Ok(options) => options,

        // No load options were passed. We will return an empty vector.
        Err(LoadOptionsError::NotSet) => {
            return Ok(Vec::new());
        }

        Err(LoadOptionsError::NotAligned) => {
            bail!("load options are not properly aligned");
        }

        Err(LoadOptionsError::InvalidString(error)) => {
            bail!("load options are not a valid string: {}", error);
        }
    };

    // Convert the options to a string.
    let options = options.to_string();

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
                .map(|c| c < 0x1f as char || c == '`')
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

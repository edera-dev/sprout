use alloc::string::{String, ToString};
use alloc::vec::Vec;
use anyhow::{Context, Result, bail};
use uefi::proto::loaded_image::{LoadOptionsError, LoadedImage};

/// Loads the command-line arguments passed to Sprout.
pub fn args() -> Result<Vec<String>> {
    // Acquire the image handle of Sprout.
    let handle = uefi::boot::image_handle();

    // Open the LoadedImage protocol for Sprout.
    let loaded_image = uefi::boot::open_protocol_exclusive::<LoadedImage>(handle)
        .context("unable to open loaded image protocol for sprout")?;

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
    // If shlex fails, we will fall back to a simple whitespace split.
    let mut args = shlex::split(&options).unwrap_or_else(|| {
        options
            .split_ascii_whitespace()
            .map(|string| string.to_string())
            .collect::<Vec<_>>()
    });

    // If there is a first argument, check if it is not an option.
    // If it is not, we will assume it is the path to the executable and remove it.
    if let Some(arg) = args.first()
        && !arg.starts_with('-')
    {
        args.remove(0);
    }

    // Correct firmware that may add invalid arguments at the start.
    // Witnessed this on a Dell Precision 5690 when direct booting.
    loop {
        // Grab the first argument or break.
        let Some(arg) = args.first() else {
            break;
        };

        // Check if the argument is a valid character.
        // If it is not, remove it and continue.
        let Some(first_character) = arg.chars().next() else {
            break;
        };

        // If the character is not a printable character or a backtick, remove it and continue.
        if first_character < 0x1f as char || first_character == '`' {
            args.remove(0);
            continue;
        }
        break;
    }

    Ok(args)
}

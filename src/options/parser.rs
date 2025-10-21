use crate::options::SproutOptions;
use anyhow::{Context, Result, bail};
use std::collections::BTreeMap;

/// For minimalism, we don't want a full argument parser. Instead, we use
/// a simple --xyz = xyz: None and --abc 123 = abc: Some("123") format.
/// We also support --abc=123 = abc: Some("123") format.
fn parse_raw() -> Result<BTreeMap<String, Option<String>>> {
    // Collect all the arguments to Sprout.
    // Skip the first argument which is the path to our executable.
    let args = std::env::args().skip(1).collect::<Vec<_>>();

    // Represent options as key-value pairs.
    let mut options = BTreeMap::new();

    // Iterators makes this way easier.
    let mut iterator = args.into_iter().peekable();

    loop {
        // Consume the next option, if any.
        let Some(option) = iterator.next() else {
            break;
        };

        // If the doesn't start with --, that is invalid.
        if !option.starts_with("--") {
            bail!("invalid option: {option}");
        }

        // Strip the -- prefix off.
        let mut option = option["--".len()..].trim().to_string();

        // An optional value.
        let mut value = None;

        // Check if the option is of the form --abc=123
        if option.contains("=") {
            let Some((part_key, part_value)) = option.split_once("=") else {
                bail!("invalid option: {option}");
            };

            let part_key = part_key.to_string();
            let part_value = part_value.to_string();
            option = part_key;
            value = Some(part_value);
        }

        if value.is_none() {
            // Check for the next value.
            let maybe_next = iterator.peek();

            // If the next value isn't another option, set the value to the next value.
            // Otherwise, it is an empty string.
            value = if let Some(next) = maybe_next
                && !next.starts_with("--")
            {
                iterator.next()
            } else {
                None
            };
        }

        // Error on empty option names.
        if option.is_empty() {
            bail!("invalid empty option: {option}");
        }

        // Insert the option and the value into the map.
        options.insert(option, value);
    }
    Ok(options)
}

/// Parse the arguments to Sprout as a [SproutOptions] structure.
pub fn parse() -> anyhow::Result<SproutOptions> {
    // Use the default value of sprout options and have the raw options be parsed into it.
    let mut result = SproutOptions::default();
    let options = parse_raw().context("unable to parse options")?;

    for (key, value) in options {
        match key.as_str() {
            "config" => {
                // The configuration file to load.
                result.config = value.context("--config option requires a value")?;
            }

            "boot" => {
                // The entry to boot.
                result.boot = Some(value.context("--boot option requires a value")?);
            }

            _ => bail!("unknown option: --{key}"),
        }
    }
    Ok(result)
}

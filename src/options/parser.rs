use anyhow::{Context, Result, bail};
use std::collections::BTreeMap;

/// The type of option. This disambiguates different behavior
/// of how options are handled.
#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub enum OptionForm {
    /// A flag, like --verbose.
    Flag,
    /// A value, in the form --abc 123 or --abc=123.
    Value,
    /// Help flag, like --help.
    Help,
}

/// The description of an option, used in the options parser
/// to make decisions about how to progress.
#[derive(Debug, Clone)]
pub struct OptionDescription<'a> {
    /// The description of the option.
    pub description: &'a str,
    /// The type of option to parse as.
    pub form: OptionForm,
}

/// Represents a type that can be parsed from command line arguments.
/// This is a super minimal options parser mechanism just for Sprout.
pub trait OptionsRepresentable {
    /// The output type that parsing will produce.
    type Output;

    /// The configured options for this type. This should describe all the options
    /// that are valid to produce the type. The left hand side is the name of the option,
    /// and the right hand side is the description.
    fn options() -> &'static [(&'static str, OptionDescription<'static>)];

    /// Produces the type by taking the `options` and processing it into the output.
    fn produce(options: BTreeMap<String, Option<String>>) -> Result<Self::Output>;

    /// For minimalism, we don't want a full argument parser. Instead, we use
    /// a simple --xyz = xyz: None and --abc 123 = abc: Some("123") format.
    /// We also support --abc=123 = abc: Some("123") format.
    fn parse_raw() -> Result<BTreeMap<String, Option<String>>> {
        // Access the configured options for this type.
        let configured: BTreeMap<_, _> = BTreeMap::from_iter(Self::options().to_vec());

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

            // Error on empty option names.
            if option.is_empty() {
                bail!("invalid empty option");
            }

            // Find the description of the configured option, if any.
            let Some(description) = configured.get(option.as_str()) else {
                bail!("invalid option: --{option}");
            };

            // Check if the option requires a value and error if none was provided.
            if description.form == OptionForm::Value && value.is_none() {
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

            // If the option form does not support a value and there is a value, error.
            if description.form != OptionForm::Value && value.is_some() {
                bail!("option --{} does not take a value", option);
            }

            // Handle the --help flag case.
            if description.form == OptionForm::Help {
                // Generic configured options output.
                println!("Configured Options:");
                for (name, description) in &configured {
                    println!(
                        "  --{}{}: {}",
                        name,
                        if description.form == OptionForm::Value {
                            " <value>"
                        } else {
                            ""
                        },
                        description.description
                    );
                }
                // Exit because the help has been displayed.
                std::process::exit(1);
            }

            // Insert the option and the value into the map.
            options.insert(option, value);
        }
        Ok(options)
    }

    /// Parses the program arguments as a [Self::Output], calling [Self::parse_raw] and [Self::produce].
    fn parse() -> Result<Self::Output> {
        // Parse the program arguments into a raw map.
        let options = Self::parse_raw().context("unable to parse options")?;
        // Produce the options from the map.
        Self::produce(options)
    }
}

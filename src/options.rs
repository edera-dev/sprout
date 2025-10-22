use crate::options::parser::{OptionDescription, OptionForm, OptionsRepresentable};
use anyhow::{Context, Result, bail};
use std::collections::BTreeMap;

/// The Sprout options parser.
pub mod parser;

/// Default configuration file path.
const DEFAULT_CONFIG_PATH: &str = "\\sprout.toml";

/// The parsed options of sprout.
#[derive(Debug)]
pub struct SproutOptions {
    /// Path to a configuration file to load.
    pub config: String,
    /// Entry to boot without showing the boot menu.
    pub boot: Option<String>,
}

/// The default Sprout options.
impl Default for SproutOptions {
    fn default() -> Self {
        Self {
            config: DEFAULT_CONFIG_PATH.to_string(),
            boot: None,
        }
    }
}

/// The options parser mechanism for Sprout.
impl OptionsRepresentable for SproutOptions {
    /// Produce the [SproutOptions] structure.
    type Output = Self;

    /// All the Sprout options that are defined.
    fn options() -> &'static [(&'static str, OptionDescription<'static>)] {
        &[
            (
                "config",
                OptionDescription {
                    description: "Path to Sprout configuration file",
                    form: OptionForm::Value,
                },
            ),
            (
                "boot",
                OptionDescription {
                    description: "Entry to boot, bypassing the menu",
                    form: OptionForm::Value,
                },
            ),
            (
                "help",
                OptionDescription {
                    description: "Display Sprout Help",
                    form: OptionForm::Help,
                },
            ),
        ]
    }

    /// Produces [SproutOptions] from the parsed raw `options` map.
    fn produce(options: BTreeMap<String, Option<String>>) -> Result<Self> {
        // Use the default value of sprout options and have the raw options be parsed into it.
        let mut result = Self::default();

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
}

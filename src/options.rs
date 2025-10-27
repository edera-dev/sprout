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
    /// Configures Sprout automatically based on the environment.
    pub autoconfigure: bool,
    /// Path to a configuration file to load.
    pub config: String,
    /// Entry to boot without showing the boot menu.
    pub boot: Option<String>,
    /// Force display of the boot menu.
    pub force_menu: bool,
    /// The timeout for the boot menu in seconds.
    pub menu_timeout: Option<u64>,
}

/// The default Sprout options.
impl Default for SproutOptions {
    fn default() -> Self {
        Self {
            autoconfigure: false,
            config: DEFAULT_CONFIG_PATH.to_string(),
            boot: None,
            force_menu: false,
            menu_timeout: None,
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
                "force-menu",
                OptionDescription {
                    description: "Force showing of the boot menu",
                    form: OptionForm::Flag,
                },
            ),
            (
                "menu-timeout",
                OptionDescription {
                    description: "Boot menu timeout, in seconds",
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
                "autoconfigure" => {
                    // Enable autoconfiguration.
                    result.autoconfigure = true;
                }

                "config" => {
                    // The configuration file to load.
                    result.config = value.context("--config option requires a value")?;
                }

                "boot" => {
                    // The entry to boot.
                    result.boot = Some(value.context("--boot option requires a value")?);
                }

                "force-menu" => {
                    // Force showing of the boot menu.
                    result.force_menu = true;
                }

                "menu-timeout" => {
                    // The timeout for the boot menu in seconds.
                    let value = value.context("--menu-timeout option requires a value")?;
                    let value = value
                        .parse::<u64>()
                        .context("menu-timeout must be a number")?;
                    result.menu_timeout = Some(value);
                }

                _ => bail!("unknown option: --{key}"),
            }
        }
        Ok(result)
    }
}

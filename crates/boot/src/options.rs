use alloc::string::{String, ToString};
use anyhow::{Context, Result, bail};
use core::ptr::null_mut;
use jaarg::alloc::ParseMapResult;
use jaarg::{
    ErrorUsageWriter, ErrorUsageWriterContext, HelpWriter, HelpWriterContext, Opt, Opts,
    StandardErrorUsageWriter, StandardFullHelpWriter,
};
use log::{error, info};
use uefi_raw::Status;

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
impl SproutOptions {
    /// Produces [SproutOptions] from the arguments provided by the UEFI core.
    /// Internally we utilize the `jaarg` argument parser which has excellent no_std support.
    pub fn parse() -> Result<Self> {
        // All the options for the Sprout executable.
        const OPTIONS: Opts<&str> = Opts::new(&[
            Opt::help_flag("help", &["--help"]).help_text("Display Sprout Help"),
            Opt::flag("autoconfigure", &["--autoconfigure"])
                .help_text("Enable Sprout autoconfiguration"),
            Opt::value("config", &["--config"], "PATH")
                .help_text("Path to Sprout configuration file"),
            Opt::value("boot", &["--boot"], "ENTRY").help_text("Entry to boot, bypassing the menu"),
            Opt::flag("force-menu", &["--force-menu"]).help_text("Force showing the boot menu"),
            Opt::value("menu-timeout", &["--menu-timeout"], "TIMEOUT")
                .help_text("Boot menu timeout, in seconds"),
        ]);

        // Acquire the arguments as determined by the UEFI core.
        let args = eficore::env::args()?;

        // Parse the OPTIONS into a map using jaarg.
        let parsed = match OPTIONS.parse_map(
            "sprout",
            args.iter(),
            |program_name| {
                let ctx = HelpWriterContext {
                    options: &OPTIONS,
                    program_name,
                };
                info!("{}", StandardFullHelpWriter::new(ctx));
            },
            |program_name, error| {
                let ctx = ErrorUsageWriterContext {
                    options: &OPTIONS,
                    program_name,
                    error,
                };
                error!("{}", StandardErrorUsageWriter::new(ctx));
            },
        ) {
            ParseMapResult::Map(map) => map,
            ParseMapResult::ExitSuccess => unsafe {
                uefi::boot::exit(uefi::boot::image_handle(), Status::SUCCESS, 0, null_mut());
            },

            ParseMapResult::ExitFailure => unsafe {
                uefi::boot::exit(uefi::boot::image_handle(), Status::ABORTED, 0, null_mut());
            },
        };

        // Use the default value of sprout options and have the raw options be parsed into it.
        let mut result = Self::default();

        for (key, value) in parsed {
            match key {
                "autoconfigure" => {
                    // Enable autoconfiguration.
                    result.autoconfigure = true;
                }

                "config" => {
                    // The configuration file to load.
                    result.config = value;
                }

                "boot" => {
                    // The entry to boot.
                    result.boot = Some(value);
                }

                "force-menu" => {
                    // Force showing of the boot menu.
                    result.force_menu = true;
                }

                "menu-timeout" => {
                    // The timeout for the boot menu in seconds.
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

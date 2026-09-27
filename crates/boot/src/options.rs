use alloc::string::{String, ToString};
use anyhow::{Context, Result, anyhow, bail};
use core::cell::Cell;
use core::ptr::null_mut;
use edera_sprout_config::MenuStyle;
use jaarg::{
    HelpWriter, HelpWriterContext, Opt, Opts, ParseControl, ParseResult, StandardFullHelpWriter,
};
use log::{info, warn};
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
    /// The style of boot menu to display.
    pub menu_style: Option<MenuStyle>,
    /// Retains the boot console before boot.
    pub retain_boot_console: bool,
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
            menu_style: None,
            retain_boot_console: false,
        }
    }
}

/// The options parser mechanism for Sprout.
impl SproutOptions {
    /// Produces [SproutOptions] from the arguments provided by the UEFI core.
    /// Internally, we use the `jaarg` argument parser which has excellent no_std support.
    pub fn parse() -> Result<Self> {
        enum ArgID {
            Help,
            AutoConfigure,
            Config,
            Boot,
            ForceMenu,
            MenuTimeout,
            MenuStyle,
            RetainBootConsole,
        }

        // All the options for the Sprout executable.
        const OPTIONS: Opts<ArgID> = Opts::new(&[
            Opt::help_flag(ArgID::Help, &["--help"]).help_text("Display Sprout Help"),
            Opt::flag(ArgID::AutoConfigure, &["--autoconfigure"])
                .help_text("Enable Sprout autoconfiguration"),
            Opt::value(ArgID::Config, &["--config"], "PATH")
                .help_text("Path to Sprout configuration file"),
            Opt::value(ArgID::Boot, &["--boot"], "ENTRY")
                .help_text("Entry to boot, bypassing the menu"),
            Opt::flag(ArgID::ForceMenu, &["--force-menu"]).help_text("Force showing the boot menu"),
            Opt::value(ArgID::MenuTimeout, &["--menu-timeout"], "TIMEOUT")
                .help_text("Boot menu timeout, in seconds"),
            Opt::value(ArgID::MenuStyle, &["--menu-style"], "STYLE")
                .help_text("Boot menu style, basic or simple"),
            Opt::flag(ArgID::RetainBootConsole, &["--retain-boot-console"])
                .help_text("Retain boot console before boot"),
        ]);

        // Acquire the arguments as determined by the UEFI core.
        let mut args = eficore::env::args()?;

        // Parse the arguments, dropping any argument that can't be parsed and parsing again.
        // A typo in a boot entry or junk from the firmware should not prevent booting.
        loop {
            // Use the default value of sprout options and have the raw options be parsed into it.
            let mut result = Self::default();

            // The number of arguments handed to the parser, used to find the one it rejected.
            let consumed = Cell::new(0usize);
            // The reason the parser rejected an argument, if it did.
            let mut rejection = None;

            // Parse the OPTIONS into a map using jaarg.
            let parsed = OPTIONS.parse(
                "sprout",
                args.iter().inspect(|_| consumed.set(consumed.get() + 1)),
                |program_name, id, _opt, name, value| {
                    match id {
                        ArgID::AutoConfigure => {
                            // Enable autoconfiguration.
                            result.autoconfigure = true;
                        }
                        ArgID::Config => {
                            // The configuration file to load.
                            result.config = value.into();
                        }
                        ArgID::Boot => {
                            // The entry to boot.
                            result.boot = Some(value.into());
                        }
                        ArgID::ForceMenu => {
                            // Force showing of the boot menu.
                            result.force_menu = true;
                        }
                        ArgID::MenuTimeout => {
                            // The timeout for the boot menu in seconds.
                            match value.parse::<u64>() {
                                Ok(timeout) => result.menu_timeout = Some(timeout),
                                Err(error) => {
                                    warn!("ignoring invalid {} value '{}': {}", name, value, error)
                                }
                            }
                        }
                        ArgID::MenuStyle => {
                            // The style of boot menu to display.
                            match value {
                                "basic" => result.menu_style = Some(MenuStyle::Basic),
                                "simple" => result.menu_style = Some(MenuStyle::Simple),
                                _ => warn!(
                                    "ignoring invalid {} value '{}': expected basic or simple",
                                    name, value
                                ),
                            }
                        }
                        ArgID::RetainBootConsole => {
                            // Retain the boot console before booting.
                            result.retain_boot_console = true;
                        }
                        ArgID::Help => {
                            let ctx = HelpWriterContext {
                                options: &OPTIONS,
                                program_name,
                            };
                            info!("{}", StandardFullHelpWriter::new(ctx));
                            return Ok(ParseControl::Quit);
                        }
                    }
                    Ok(ParseControl::Continue)
                },
                |_program_name, error| {
                    rejection = Some(error.to_string());
                },
            );

            match parsed {
                ParseResult::ContinueSuccess => return Ok(result),
                ParseResult::ExitSuccess => unsafe {
                    uefi::boot::exit(uefi::boot::image_handle(), Status::SUCCESS, 0, null_mut())
                        .context("unable to exit")?;
                    return Err(anyhow!("unable to exit"));
                },

                ParseResult::ExitError => {
                    // The parser stops at the argument it rejected, which makes it the last
                    // argument it was handed. This is also true of an option missing its value,
                    // which is only detected once the arguments run out.
                    let Some(index) = consumed.get().checked_sub(1).filter(|i| *i < args.len())
                    else {
                        bail!("unable to parse options: {}", rejection.unwrap_or_default());
                    };
                    let arg = args.remove(index);
                    warn!(
                        "ignoring argument '{}': {}",
                        arg,
                        rejection.unwrap_or_default()
                    );
                }
            }
        }
    }
}

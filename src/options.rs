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

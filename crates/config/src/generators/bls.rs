use crate::entries::EntryDeclaration;
use alloc::string::{String, ToString};
use serde::{Deserialize, Serialize};

/// The default path to the BLS directory.
const BLS_TEMPLATE_PATH: &str = "\\loader";

/// The configuration of the BLS generator.
/// The BLS uses the Bootloader Specification to produce
/// entries from an input template.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlsConfiguration {
    /// The entry to use for as a template.
    pub entry: EntryDeclaration,
    /// The path to the BLS directory.
    #[serde(default = "default_bls_path")]
    pub path: String,
    /// Whether generated entries keep the name of the BLS entry file as-is.
    /// When enabled, the generator name is not prepended, so the entry names match
    /// the BLS entry ids used by tools like `bootctl set-default`. Variants still append
    /// their choice names, so entries of a generator with variants never match the ids
    /// exactly, and are best selected with a pattern like `<id>-*`. Two BLS generators
    /// that read the same BLS directory will produce entries with the same names, so
    /// generators that reuse a BLS directory should disable this.
    #[serde(default = "default_pin_names", rename = "pin-names")]
    pub pin_names: bool,
}

/// The default configuration, which matches an empty BLS generator section.
impl Default for BlsConfiguration {
    fn default() -> Self {
        Self {
            entry: Default::default(),
            path: default_bls_path(),
            pin_names: default_pin_names(),
        }
    }
}

fn default_pin_names() -> bool {
    true
}

fn default_bls_path() -> String {
    BLS_TEMPLATE_PATH.to_string()
}

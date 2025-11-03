use crate::entries::EntryDeclaration;
use alloc::string::{String, ToString};
use serde::{Deserialize, Serialize};

/// The default path to the BLS directory.
const BLS_TEMPLATE_PATH: &str = "\\loader";

/// The configuration of the BLS generator.
/// The BLS uses the Bootloader Specification to produce
/// entries from an input template.
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct BlsConfiguration {
    /// The entry to use for as a template.
    pub entry: EntryDeclaration,
    /// The path to the BLS directory.
    #[serde(default = "default_bls_path")]
    pub path: String,
}

fn default_bls_path() -> String {
    BLS_TEMPLATE_PATH.to_string()
}

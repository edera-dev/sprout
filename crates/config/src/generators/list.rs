use crate::entries::EntryDeclaration;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// List generator configuration.
/// The list generator produces multiple entries based
/// on a set of input maps.
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct ListConfiguration {
    /// The template entry to use for each generated entry.
    #[serde(default)]
    pub entry: EntryDeclaration,
    /// The values to use as the input for the matrix.
    #[serde(default)]
    pub values: Vec<BTreeMap<String, String>>,
}

use crate::entries::EntryDeclaration;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Matrix generator configuration.
/// The matrix generator produces multiple entries based
/// on input values multiplicatively.
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct MatrixConfiguration {
    /// The template entry to use for each generated entry.
    #[serde(default)]
    pub entry: EntryDeclaration,
    /// The values to use as the input for the matrix.
    #[serde(default)]
    pub values: BTreeMap<String, Vec<String>>,
}

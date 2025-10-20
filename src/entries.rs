use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Declares a boot entry to display in the boot menu.
///
/// Entries are the user-facing concept of Sprout, making it possible
/// to run a set of actions with a specific context.
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct EntryDeclaration {
    /// The title of the entry which will be display in the boot menu.
    pub title: String,
    /// The actions to run when the entry is selected.
    #[serde(default)]
    pub actions: Vec<String>,
    /// The values to insert into the context when the entry is selected.
    #[serde(default)]
    pub values: BTreeMap<String, String>,
}

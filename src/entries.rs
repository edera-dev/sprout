use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct EntryDeclaration {
    pub title: String,
    #[serde(default)]
    pub actions: Vec<String>,
    #[serde(default)]
    pub values: BTreeMap<String, String>,
}

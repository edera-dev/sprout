use alloc::collections::BTreeMap;
use alloc::string::String;
use serde::{Deserialize, Serialize};

/// A single choice of a variant axis.
/// See [crate::generators::GeneratorDeclaration::variants] for how variants are applied.
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct VariantChoice {
    /// The name of the choice. This is set as the value of the axis name,
    /// and is appended to the name of each entry the choice applies to.
    pub name: String,
    /// The values to insert into the context of each entry the choice applies to.
    #[serde(default)]
    pub values: BTreeMap<String, String>,
}

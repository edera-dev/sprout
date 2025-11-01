use crate::context::SproutContext;
use crate::entries::{BootableEntry, EntryDeclaration};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::rc::Rc;

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

/// Generates a set of entries using the specified `list` configuration in the `context`.
pub fn generate(
    context: Rc<SproutContext>,
    list: &ListConfiguration,
) -> Result<Vec<BootableEntry>> {
    let mut entries = Vec::new();

    // For each combination, create a new context and entry.
    for (index, combination) in list.values.iter().enumerate() {
        let mut context = context.fork();
        // Insert the combination into the context.
        context.insert(combination);
        let context = context.freeze();

        // Stamp the entry title and actions from the template.
        let mut entry = list.entry.clone();
        entry.actions = entry
            .actions
            .into_iter()
            .map(|action| context.stamp(action))
            .collect();
        // Push the entry into the list with the new context.
        entries.push(BootableEntry::new(
            index.to_string(),
            entry.title.clone(),
            context,
            entry,
        ));
    }

    Ok(entries)
}

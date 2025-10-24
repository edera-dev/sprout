use crate::context::SproutContext;
use crate::entries::{BootableEntry, EntryDeclaration};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::rc::Rc;

/// Matrix generator configuration.
/// The matrix generator produces multiple entries based
/// on input values multiplicatively.
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct MatrixConfiguration {
    /// The template entry to use for each generated entry.
    #[serde(default)]
    pub entry: EntryDeclaration,
    /// The values to use as the input for the matrix.
    #[serde(default)]
    pub values: BTreeMap<String, Vec<String>>,
}

/// Builds out multiple generations of `input` based on a matrix style.
/// For example, if input is: {"x": ["a", "b"], "y": ["c", "d"]}
/// It will produce:
/// x: a, y: c
/// x: a, y: d
/// x: b, y: c
/// x: b, y: d
fn build_matrix(input: &BTreeMap<String, Vec<String>>) -> Vec<BTreeMap<String, String>> {
    // Convert the input into a vector of tuples.
    let items: Vec<(String, Vec<String>)> = input.clone().into_iter().collect();

    // The result is a vector of maps.
    let mut result: Vec<BTreeMap<String, String>> = vec![BTreeMap::new()];

    for (key, values) in items {
        let mut new_result = Vec::new();

        // Produce all the combinations of the input values.
        for combination in &result {
            for value in &values {
                let mut new_combination = combination.clone();
                new_combination.insert(key.clone(), value.clone());
                new_result.push(new_combination);
            }
        }

        result = new_result;
    }

    result.into_iter().filter(|item| !item.is_empty()).collect()
}

/// Generates a set of entries using the specified `matrix` configuration in the `context`.
pub fn generate(
    context: Rc<SproutContext>,
    matrix: &MatrixConfiguration,
) -> Result<Vec<BootableEntry>> {
    // Produce all the combinations of the input values.
    let combinations = build_matrix(&matrix.values);
    let mut entries = Vec::new();

    // For each combination, create a new context and entry.
    for (index, combination) in combinations.into_iter().enumerate() {
        let mut context = context.fork();
        // Insert the combination into the context.
        context.insert(&combination);
        let context = context.freeze();

        // Stamp the entry title and actions from the template.
        let mut entry = matrix.entry.clone();
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

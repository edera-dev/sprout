use crate::context::SproutContext;
use crate::entries::BootableEntry;
use crate::generators::list;
use alloc::collections::BTreeMap;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use anyhow::Result;
use edera_sprout_config::generators::list::ListConfiguration;
use edera_sprout_config::generators::matrix::MatrixConfiguration;

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
    // Use the list generator to generate entries for each combination.
    list::generate(
        context,
        &ListConfiguration {
            entry: matrix.entry.clone(),
            values: combinations,
        },
    )
}

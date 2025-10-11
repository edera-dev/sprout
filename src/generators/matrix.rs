use crate::config::EntryDeclaration;
use crate::context::Context;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::rc::Rc;

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct MatrixConfiguration {
    #[serde(default)]
    pub entry: EntryDeclaration,
    #[serde(default)]
    pub values: BTreeMap<String, Vec<String>>,
}

fn build_matrix(input: &BTreeMap<String, Vec<String>>) -> Vec<BTreeMap<String, String>> {
    let items: Vec<(String, Vec<String>)> = input.clone().into_iter().collect();
    let mut result: Vec<BTreeMap<String, String>> = vec![BTreeMap::new()];

    for (key, values) in items {
        let mut new_result = Vec::new();

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

pub fn generate(
    context: Rc<Context>,
    matrix: &MatrixConfiguration,
) -> Vec<(Rc<Context>, EntryDeclaration)> {
    let combinations = build_matrix(&matrix.values);
    let mut entries = Vec::new();

    for combination in combinations {
        let mut context = context.fork();
        context.insert(&combination);
        let context = context.freeze();

        let mut entry = matrix.entry.clone();
        entry.title = context.stamp(&entry.title);
        entry.actions = entry
            .actions
            .into_iter()
            .map(|action| context.stamp(action))
            .collect();
        entries.push((context, entry));
    }

    entries
}

use crate::context::SproutContext;
use crate::entries::BootableEntry;
use alloc::collections::BTreeSet;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use anyhow::Result;
use anyhow::bail;
use edera_sprout_config::generators::GeneratorDeclaration;
use edera_sprout_parsing::{build_variants, rule_matches};

/// The BLS generator.
pub mod bls;

/// The list generator.
pub mod list;

/// The matrix generator.
pub mod matrix;

/// Runs the generator kind specified by the `generator` option.
fn generate_base(
    context: Rc<SproutContext>,
    generator: &GeneratorDeclaration,
) -> Result<Vec<BootableEntry>> {
    if let Some(matrix) = &generator.matrix {
        matrix::generate(context, matrix)
    } else if let Some(bls) = &generator.bls {
        bls::generate(context, bls)
    } else if let Some(list) = &generator.list {
        list::generate(context, list)
    } else {
        bail!("unknown generator configuration");
    }
}

/// Multiplies `entries` by every combination of the variants in `generator`.
fn apply_variants(
    entries: Vec<BootableEntry>,
    generator: &GeneratorDeclaration,
) -> Result<Vec<BootableEntry>> {
    // Without any variants, the entries are used as-is.
    if generator.variants.is_empty() {
        return Ok(entries);
    }

    // Validate the axes, as an axis without choices would silently produce no entries,
    // and duplicate choice names would produce entries with the same name.
    let mut axes = Vec::new();
    for (axis, choices) in &generator.variants {
        if choices.is_empty() {
            bail!("variant axis '{}' has no choices", axis);
        }
        let mut seen = BTreeSet::new();
        for choice in choices {
            if !seen.insert(choice.name.as_str()) {
                bail!(
                    "variant axis '{}' has more than one choice named '{}'",
                    axis,
                    choice.name
                );
            }
        }
        axes.push((
            axis.clone(),
            choices
                .iter()
                .map(|choice| (choice.name.clone(), choice.values.clone()))
                .collect(),
        ));
    }

    let combinations = build_variants(&axes);
    let mut result = Vec::with_capacity(entries.len() * combinations.len());
    for entry in &entries {
        for combination in &combinations {
            let mut context = entry.context().fork();
            context.insert(&combination.values);
            let suffix: String = combination
                .names
                .iter()
                .map(|name| format!("-{}", name))
                .collect();
            result.push(entry.variant(&suffix, context.freeze()));
        }
    }
    Ok(result)
}

/// Runs the generator specified by the `generator` option, named `name`.
/// It uses the specified `context` as the parent context for
/// the generated entries, injecting more values if needed.
/// Entry names are prefixed with the generator name unless pinned,
/// then variants are applied, and then excluded entries are removed.
pub fn generate(
    name: &str,
    context: Rc<SproutContext>,
    generator: &GeneratorDeclaration,
) -> Result<Vec<BootableEntry>> {
    let mut entries = generate_base(context, generator)?;

    // We will prefix all entries with [name]-, provided the name is not pinned.
    let prefix = format!("{}-", name);
    for entry in &mut entries {
        if !entry.is_pin_name() {
            entry.prepend_name_prefix(&prefix);
        }
    }

    let mut entries = apply_variants(entries, generator)?;

    // Remove any entries that match an exclusion rule.
    if !generator.exclude.is_empty() {
        entries.retain(|entry| {
            let values = entry.context().all_values();
            !generator
                .exclude
                .iter()
                .any(|rule| rule_matches(rule, &values))
        });
    }

    Ok(entries)
}

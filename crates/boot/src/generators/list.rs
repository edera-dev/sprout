use crate::context::SproutContext;
use crate::entries::BootableEntry;
use alloc::rc::Rc;
use alloc::string::ToString;
use alloc::vec::Vec;
use anyhow::Result;
use edera_sprout_config::generators::list::ListConfiguration;

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

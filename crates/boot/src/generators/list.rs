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

        // The actions are stamped with the final context of the entry when it is booted,
        // so that variants and the values of the entry can override the values of the generator.
        let entry = list.entry.clone();

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

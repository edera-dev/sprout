use crate::context::SproutContext;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::rc::Rc;

/// Declares a boot entry to display in the boot menu.
///
/// Entries are the user-facing concept of Sprout, making it possible
/// to run a set of actions with a specific context.
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct EntryDeclaration {
    /// The title of the entry which will be display in the boot menu.
    /// This is the pre-stamped value.
    pub title: String,
    /// The actions to run when the entry is selected.
    #[serde(default)]
    pub actions: Vec<String>,
    /// The values to insert into the context when the entry is selected.
    #[serde(default)]
    pub values: BTreeMap<String, String>,
}

/// Represents an entry that is stamped and ready to be booted.
#[derive(Clone)]
pub struct BootableEntry {
    name: String,
    title: String,
    context: Rc<SproutContext>,
    declaration: EntryDeclaration,
}

impl BootableEntry {
    /// Create a new bootable entry to represent the full context of an entry.
    pub fn new(
        name: String,
        title: String,
        context: Rc<SproutContext>,
        declaration: EntryDeclaration,
    ) -> Self {
        Self {
            name,
            title,
            context,
            declaration,
        }
    }

    /// Fetch the name of the entry. This is usually a machine-identifiable key.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Fetch the title of the entry. This is usually a human-readable key.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Fetch the full context of the entry.
    pub fn context(&self) -> Rc<SproutContext> {
        Rc::clone(&self.context)
    }

    /// Fetch the declaration of the entry.
    pub fn declaration(&self) -> &EntryDeclaration {
        &self.declaration
    }

    /// Swap out the context of the entry.
    pub fn swap_context(&mut self, context: Rc<SproutContext>) {
        self.context = context;
    }

    /// Restamp the title with the current context.
    pub fn restamp_title(&mut self) {
        self.title = self.context.stamp(&self.title);
    }

    /// Prepend the name of the entry with `prefix`.
    pub fn prepend_name_prefix(&mut self, prefix: &str) {
        self.name.insert_str(0, prefix);
    }
}

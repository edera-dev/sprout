use crate::context::SproutContext;
use alloc::rc::Rc;
use alloc::string::{String, ToString};
use edera_sprout_config::entries::EntryDeclaration;

/// Represents an entry that is stamped and ready to be booted.
#[derive(Clone)]
pub struct BootableEntry {
    name: String,
    title: String,
    context: Rc<SproutContext>,
    declaration: EntryDeclaration,
    default: bool,
    pin_name: bool,
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
            default: false,
            pin_name: false,
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

    /// Fetch whether the entry is the default entry.
    pub fn is_default(&self) -> bool {
        self.default
    }

    /// Fetch whether the entry is pinned, which prevents prefixing.
    pub fn is_pin_name(&self) -> bool {
        self.pin_name
    }

    /// Swap out the context of the entry.
    pub fn swap_context(&mut self, context: Rc<SproutContext>) {
        self.context = context;
    }

    /// Restamp the title with the current context.
    pub fn restamp_title(&mut self) {
        self.title = self.context.stamp(&self.title);
    }

    /// Mark this entry as the default entry.
    pub fn mark_default(&mut self) {
        self.default = true;
    }

    // Unmark this entry as the default entry.
    pub fn unmark_default(&mut self) {
        self.default = false;
    }

    /// Mark this entry as being pinned, which prevents prefixing.
    pub fn mark_pin_name(&mut self) {
        self.pin_name = true;
    }

    /// Prepend the name of the entry with `prefix`.
    pub fn prepend_name_prefix(&mut self, prefix: &str) {
        self.name.insert_str(0, prefix);
    }

    /// Determine if this entry matches `needle` by comparing to the name or title of the entry.
    /// If `needle` ends with *, we will match a partial match.
    pub fn is_match(&self, needle: &str) -> bool {
        // If the needle ends with '*', we will accept a partial match.
        if needle.ends_with("*") {
            // Strip off any '*' at the end.
            let partial = needle.trim_end_matches("*");
            // Check if the name or title start with the partial match.
            return self.name.starts_with(partial) || self.title.starts_with(partial);
        }

        // Standard quality matching rules.
        self.name == needle || self.title == needle
    }

    /// Find an entry by `needle` inside the entry iterator `haystack`.
    /// This will search for an entry by name, title, or index.
    pub fn find<'a>(
        needle: &str,
        haystack: impl Iterator<Item = &'a BootableEntry>,
    ) -> Option<&'a BootableEntry> {
        haystack
            .enumerate()
            .find(|(index, entry)| entry.is_match(needle) || index.to_string() == needle)
            .map(|(_index, entry)| entry)
    }
}

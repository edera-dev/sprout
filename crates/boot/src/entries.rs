use crate::boot_counter::BootCounterTarget;
use crate::context::SproutContext;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::{String, ToString};
use edera_sprout_config::entries::EntryDeclaration;
use edera_sprout_parsing::glob_match;

/// Represents an entry that is stamped and ready to be booted.
#[derive(Clone)]
pub struct BootableEntry {
    name: String,
    title: String,
    context: Rc<SproutContext>,
    declaration: EntryDeclaration,
    default: bool,
    pin_name: bool,
    sort_key: Option<String>,
    boot_counter: Option<BootCounterTarget>,
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
            sort_key: None,
            boot_counter: None,
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

    /// Restamp the declared sort key with the current context.
    /// A sort key that was set on this entry directly is left as is.
    pub fn restamp_sort_key(&mut self) {
        if self.sort_key.is_none()
            && let Some(ref sort_key) = self.declaration.sort_key
        {
            self.sort_key = Some(self.context.stamp(sort_key));
        }
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

    /// Record where the boot counter of this entry is stored.
    pub fn set_boot_counter(&mut self, target: BootCounterTarget) {
        self.boot_counter = Some(target);
    }

    /// Fetch where the boot counter of this entry is stored, if it has one.
    pub fn boot_counter(&self) -> Option<&BootCounterTarget> {
        self.boot_counter.as_ref()
    }

    /// Fetch whether the entry has a boot counter with no tries left.
    pub fn is_bad(&self) -> bool {
        self.boot_counter
            .as_ref()
            .is_some_and(|target| target.counter.is_bad())
    }

    /// Prepend the name of the entry with `prefix`.
    pub fn prepend_name_prefix(&mut self, prefix: &str) {
        self.name.insert_str(0, prefix);
    }

    /// Determine if this entry matches `needle` by comparing to the name or title of the entry.
    /// The `needle` is a glob pattern, where `*` matches any sequence of characters.
    /// A `needle` without any `*` must equal the name or title exactly.
    pub fn is_match(&self, needle: &str) -> bool {
        glob_match(needle, &self.name) || glob_match(needle, &self.title)
    }

    /// Determine if this entry matches `needle` by comparing to the id of the entry, which is
    /// the name with or without the `.conf` suffix. This is how loader.conf selects entries.
    /// The `needle` is a glob pattern, where `*` matches any sequence of characters.
    pub fn is_match_id(&self, needle: &str) -> bool {
        glob_match(needle, &self.name) || glob_match(needle, &format!("{}.conf", self.name))
    }

    /// Create a variant of this entry, with the name suffixed by `suffix` and using `context`.
    /// The actions of the entry are stamped with `context`, the same as the list generator.
    /// Variants of the same entry keep the sort key of this entry, so they stay grouped
    /// together in the order they were created in.
    pub fn variant(&self, suffix: &str, context: Rc<SproutContext>) -> Self {
        let mut entry = self.clone();
        entry.name.push_str(suffix);
        entry.declaration.actions = context
            .stamp_iter(entry.declaration.actions.iter())
            .collect();
        // Without any sort key, the name is used to sort, which differs between variants.
        // Pin the sort key to the name of this entry to keep the variants together.
        if entry.sort_key.is_none() && entry.declaration.sort_key.is_none() {
            entry.sort_key = Some(self.name.clone());
        }
        entry.context = context;
        entry
    }

    /// Set the sort key of the entry. This is used to sort entries via version comparison.
    pub fn set_sort_key(&mut self, sort_key: String) {
        self.sort_key = Some(sort_key);
    }

    /// Retrieve a reference to the sort key of the entry. If one is not specified, we will use the
    /// name of the entry.
    pub fn sort_key(&self) -> &str {
        // Use the sort key specified in the bootable entry, or use the declaration sort key,
        // or use the name of the entry.
        self.sort_key
            .as_deref()
            .or(self.declaration.sort_key.as_deref())
            .unwrap_or(&self.name)
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

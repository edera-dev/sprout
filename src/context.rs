use crate::actions::ActionDeclaration;
use crate::options::SproutOptions;
use anyhow::anyhow;
use anyhow::{Result, bail};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use uefi::proto::device_path::DevicePath;

/// The maximum number of iterations that can be performed in [SproutContext::finalize].
const CONTEXT_FINALIZE_ITERATION_LIMIT: usize = 100;

/// Declares a root context for Sprout.
/// This contains data that needs to be shared across Sprout.
#[derive(Default)]
pub struct RootContext {
    /// The actions that are available in Sprout.
    actions: BTreeMap<String, ActionDeclaration>,
    /// The device path of the loaded Sprout image.
    loaded_image_path: Option<Box<DevicePath>>,
    /// The global options of Sprout.
    options: SproutOptions,
}

impl RootContext {
    /// Creates a new root context with the `loaded_image_device_path` which will be stored
    /// in the context for easy access.
    pub fn new(loaded_image_device_path: Box<DevicePath>, options: SproutOptions) -> Self {
        Self {
            actions: BTreeMap::new(),
            loaded_image_path: Some(loaded_image_device_path),
            options,
        }
    }

    /// Access the actions configured inside Sprout.
    pub fn actions(&self) -> &BTreeMap<String, ActionDeclaration> {
        &self.actions
    }

    /// Access the actions configured inside Sprout mutably for modification.
    pub fn actions_mut(&mut self) -> &mut BTreeMap<String, ActionDeclaration> {
        &mut self.actions
    }

    /// Access the device path of the loaded Sprout image.
    pub fn loaded_image_path(&self) -> Result<&DevicePath> {
        self.loaded_image_path
            .as_deref()
            .ok_or_else(|| anyhow!("no loaded image path"))
    }

    /// Access the global Sprout options.
    pub fn options(&self) -> &SproutOptions {
        &self.options
    }
}

/// A context of Sprout. This is passed around different parts of Sprout and represents
/// a [RootContext] which is data that is shared globally, and [SproutContext] which works
/// sort of like a tree of values. You can cheaply clone a [SproutContext] and modify it with
/// new values, which override the values of contexts above it.
///
/// This is a core part of the value mechanism in Sprout which makes templating possible.
pub struct SproutContext {
    root: Rc<RootContext>,
    parent: Option<Rc<SproutContext>>,
    values: BTreeMap<String, String>,
}

impl SproutContext {
    /// Create a new [SproutContext] using `root` as the root context.
    pub fn new(root: RootContext) -> Self {
        Self {
            root: Rc::new(root),
            parent: None,
            values: BTreeMap::new(),
        }
    }

    /// Access the root context of this context.
    pub fn root(&self) -> &RootContext {
        self.root.as_ref()
    }

    /// Access the root context to modify it, if possible.
    pub fn root_mut(&mut self) -> Option<&mut RootContext> {
        Rc::get_mut(&mut self.root)
    }

    /// Retrieve the value specified by `key` from this context or its parents.
    /// Returns `None` if the value is not found.
    pub fn get(&self, key: impl AsRef<str>) -> Option<&String> {
        self.values.get(key.as_ref()).or_else(|| {
            self.parent
                .as_ref()
                .and_then(|parent| parent.get(key.as_ref()))
        })
    }

    /// Collects all keys that are present in this context or its parents.
    /// This is useful for iterating over all keys in a context.
    pub fn all_keys(&self) -> Vec<String> {
        let mut keys = BTreeSet::new();

        for key in self.values.keys() {
            keys.insert(key.clone());
        }

        if let Some(parent) = &self.parent {
            keys.extend(parent.all_keys());
        }
        keys.into_iter().collect()
    }

    /// Collects all values that are present in this context or its parents.
    /// This is useful for iterating over all values in a context.
    pub fn all_values(&self) -> BTreeMap<String, String> {
        let mut values = BTreeMap::new();
        for key in self.all_keys() {
            // Acquire the value from the context. Since retrieving all the keys will give us
            // a full view of the context, we can be sure that the key exists.
            let value = self.get(&key).cloned().unwrap_or_default();
            values.insert(key.clone(), value);
        }
        values
    }

    /// Sets the value `key` to the value specified by `value` in this context.
    /// If the parent context has this key, this will override that key.
    pub fn set(&mut self, key: impl AsRef<str>, value: impl ToString) {
        self.values
            .insert(key.as_ref().to_string(), value.to_string());
    }

    /// Inserts all the specified `values` into this context.
    /// These values will take precedence over its parent context.
    pub fn insert(&mut self, values: &BTreeMap<String, String>) {
        for (key, value) in values {
            self.values.insert(key.clone(), value.clone());
        }
    }

    /// Forks this context as an owned [SproutContext]. This makes it possible
    /// to cheaply modify a context without cloning the parent context map.
    /// The parent of the returned context is [self].
    pub fn fork(self: &Rc<SproutContext>) -> Self {
        Self {
            root: self.root.clone(),
            parent: Some(self.clone()),
            values: BTreeMap::new(),
        }
    }

    /// Freezes this context into a [Rc] which makes it possible to cheaply clone
    /// and makes it less easy to modify a context. This can be used to pass the context
    /// to various other parts of Sprout and ensure it won't be modified. Instead, once
    /// a context is frozen, it should be [self.fork]'d to be modified.
    pub fn freeze(self) -> Rc<SproutContext> {
        Rc::new(self)
    }

    /// Finalizes a context by producing a context with no parent that contains all the values
    /// of all parent contexts merged. This makes it possible to ensure [SproutContext] has no
    /// inheritance with other [SproutContext]s. It will still contain a [RootContext] however.
    pub fn finalize(&self) -> Result<SproutContext> {
        // Collect all the values from the context and its parents.
        let mut current_values = self.all_values();

        // To ensure that there is no possible infinite loop, we need to check
        // the number of iterations. If it exceeds 100, we bail.
        let mut iterations: usize = 0;
        loop {
            iterations += 1;

            if iterations > CONTEXT_FINALIZE_ITERATION_LIMIT {
                bail!("infinite loop detected in context finalization");
            }

            let mut did_change = false;
            let mut values = BTreeMap::new();
            for (key, value) in &current_values {
                let (changed, result) = Self::stamp_values(&current_values, value);
                if changed {
                    // If the value changed, we need to re-stamp it.
                    did_change = true;
                }
                // Insert the new value into the value map.
                values.insert(key.clone(), result);
            }
            current_values = values;

            // If the values did not change, we can stop.
            if !did_change {
                break;
            }
        }

        // Produce the final context.
        Ok(Self {
            root: self.root.clone(),
            parent: None,
            values: current_values,
        })
    }

    /// Stamps the `text` value with the specified `values` map. The returned value indicates
    /// whether the `text` has been changed and the value that was stamped and changed.
    fn stamp_values(values: &BTreeMap<String, String>, text: impl AsRef<str>) -> (bool, String) {
        let mut result = text.as_ref().to_string();
        let mut did_change = false;

        // Sort the keys by length. This is to ensure that we stamp the longest keys first.
        // If we did not do this, "$abc" could be stamped by "$a" into an invalid result.
        let mut keys = values.keys().collect::<Vec<_>>();

        // Sort by key length, reversed. This results in the longest keys appearing first.
        keys.sort_by_key(|key| Reverse(key.len()));

        for key in keys {
            // Empty keys are not supported.
            if key.is_empty() {
                continue;
            }

            // We can fetch the value from the map. It is verifiable that the key exists.
            let Some(value) = values.get(key) else {
                unreachable!("keys iterated over is collected on a map that cannot be modified");
            };

            let next_result = result.replace(&format!("${key}"), value);
            if result != next_result {
                did_change = true;
            }
            result = next_result;
        }
        (did_change, result)
    }

    /// Stamps the input `text` with all the values in this [SproutContext] and it's parents.
    /// For example, if this context contains {"a":"b"}, and the text "hello\\$a", it will produce
    /// "hello\\b" as an output string.
    pub fn stamp(&self, text: impl AsRef<str>) -> String {
        Self::stamp_values(&self.all_values(), text.as_ref()).1
    }

    /// Unloads a [SproutContext] back into an owned context. This
    /// may not succeed if something else is holding onto the value.
    pub fn unload(self: Rc<SproutContext>) -> Option<SproutContext> {
        Rc::into_inner(self)
    }
}

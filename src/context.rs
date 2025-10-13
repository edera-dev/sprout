use crate::actions::ActionDeclaration;
use anyhow::Result;
use anyhow::anyhow;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use uefi::proto::device_path::DevicePath;

#[derive(Default)]
pub struct RootContext {
    actions: BTreeMap<String, ActionDeclaration>,
    loaded_image_path: Option<Box<DevicePath>>,
}

impl RootContext {
    pub fn new(loaded_image_device_path: Box<DevicePath>) -> Self {
        RootContext {
            actions: BTreeMap::new(),
            loaded_image_path: Some(loaded_image_device_path),
        }
    }

    pub fn actions(&self) -> &BTreeMap<String, ActionDeclaration> {
        &self.actions
    }

    pub fn actions_mut(&mut self) -> &mut BTreeMap<String, ActionDeclaration> {
        &mut self.actions
    }

    pub fn loaded_image_path(&self) -> Result<&DevicePath> {
        self.loaded_image_path
            .as_deref()
            .ok_or_else(|| anyhow!("no loaded image path"))
    }
}

pub struct SproutContext {
    root: Rc<RootContext>,
    parent: Option<Rc<SproutContext>>,
    values: BTreeMap<String, String>,
}

impl SproutContext {
    pub fn new(root: RootContext) -> Self {
        Self {
            root: Rc::new(root),
            parent: None,
            values: BTreeMap::new(),
        }
    }

    pub fn root(&self) -> &RootContext {
        self.root.as_ref()
    }

    pub fn get(&self, key: impl AsRef<str>) -> Option<&String> {
        self.values.get(key.as_ref()).or_else(|| {
            self.parent
                .as_ref()
                .and_then(|parent| parent.get(key.as_ref()))
        })
    }

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

    pub fn all_values(&self) -> BTreeMap<String, String> {
        let mut values = BTreeMap::new();
        for key in self.all_keys() {
            values.insert(key.clone(), self.get(key).cloned().unwrap_or_default());
        }
        values
    }

    pub fn set(&mut self, key: impl AsRef<str>, value: impl ToString) {
        self.values
            .insert(key.as_ref().to_string(), value.to_string());
    }

    pub fn insert(&mut self, values: &BTreeMap<String, String>) {
        for (key, value) in values {
            self.values.insert(key.clone(), value.clone());
        }
    }

    pub fn fork(self: &Rc<SproutContext>) -> Self {
        Self {
            root: self.root.clone(),
            parent: Some(self.clone()),
            values: BTreeMap::new(),
        }
    }

    pub fn freeze(self) -> Rc<SproutContext> {
        Rc::new(self)
    }

    pub fn finalize(&self) -> SproutContext {
        let mut current_values = self.all_values();

        loop {
            let mut did_change = false;
            let mut values = BTreeMap::new();
            for (key, value) in &current_values {
                let (changed, result) = Self::stamp_values(&current_values, value);
                if changed {
                    did_change = true;
                }
                values.insert(key.clone(), result);
            }
            current_values = values;

            if !did_change {
                break;
            }
        }
        Self {
            root: self.root.clone(),
            parent: None,
            values: current_values,
        }
    }

    fn stamp_values(values: &BTreeMap<String, String>, text: impl AsRef<str>) -> (bool, String) {
        let mut result = text.as_ref().to_string();
        let mut did_change = false;
        for (key, value) in values {
            let next_result = result.replace(&format!("${key}"), value);
            if result != next_result {
                did_change = true;
            }
            result = next_result;
        }
        (did_change, result)
    }

    pub fn stamp(&self, text: impl AsRef<str>) -> String {
        Self::stamp_values(&self.all_values(), text.as_ref()).1
    }
}

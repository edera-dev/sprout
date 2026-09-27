//! Sprout configuration descriptions.
//! This crate provides all the configuration structures for Sprout.
#![no_std]
extern crate alloc;

use crate::actions::ActionDeclaration;
use crate::drivers::DriverDeclaration;
use crate::entries::EntryDeclaration;
use crate::extractors::ExtractorDeclaration;
use crate::generators::GeneratorDeclaration;
use crate::phases::PhasesConfiguration;
use alloc::collections::BTreeMap;
use alloc::string::String;
use serde::{Deserialize, Serialize};

pub mod actions;
pub mod drivers;
pub mod entries;
pub mod extractors;
pub mod generators;
pub mod phases;

/// This is the latest version of the sprout configuration format.
/// This must be incremented when the configuration breaks compatibility.
pub const LATEST_VERSION: u32 = 1;

/// The default timeout for the boot menu in seconds.
pub const DEFAULT_MENU_TIMEOUT_SECONDS: u64 = 10;

/// The Sprout configuration format.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RootConfiguration {
    /// The version of the configuration. This should always be declared
    /// and be the latest version that is supported. If not specified, it is assumed
    /// the configuration is the latest version.
    #[serde(default = "latest_version")]
    pub version: u32,
    /// Default options for Sprout.
    #[serde(default)]
    pub options: OptionsConfiguration,
    /// Values to be inserted into the root sprout context.
    #[serde(default)]
    pub values: BTreeMap<String, String>,
    /// Drivers to load.
    /// These drivers provide extra functionality like filesystem support to Sprout.
    /// Each driver has a name which uniquely identifies it inside Sprout.
    #[serde(default)]
    pub drivers: BTreeMap<String, DriverDeclaration>,
    /// Declares the extractors that add values to the sprout context that are calculated
    /// at runtime. Each extractor has a name which corresponds to the value it will set
    /// inside the sprout context.
    #[serde(default)]
    pub extractors: BTreeMap<String, ExtractorDeclaration>,
    /// Declares the actions that can execute operations for sprout.
    /// Actions are executable modules in sprout that take in specific structured values.
    /// Actions are responsible for ensuring that passed strings are stamped to replace values
    /// at runtime.
    /// Each action has a name that can be referenced by other base concepts like entries.
    #[serde(default)]
    pub actions: BTreeMap<String, ActionDeclaration>,
    /// Declares the entries that are displayed on the boot menu. These entries are static
    /// but can still use values from the sprout context.
    #[serde(default)]
    pub entries: BTreeMap<String, EntryDeclaration>,
    /// Declares the generators that are used to generate entries at runtime.
    /// Each generator has its own logic for generating entries, but generally they intake
    /// a template entry and stamp that template entry over some values determined at runtime.
    /// Each generator has an associated name used to differentiate it across sprout.
    #[serde(default)]
    pub generators: BTreeMap<String, GeneratorDeclaration>,
    /// Configures the various phases of sprout. This allows you to hook into specific parts
    /// of the boot process to execute actions, for example, you can show a boot splash during
    /// the early phase.
    #[serde(default)]
    pub phases: PhasesConfiguration,
}

/// Options configuration for Sprout, used when the corresponding options are not specified.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OptionsConfiguration {
    /// The entry to mark as the default entry, instead of the first entry.
    #[serde(rename = "default-entry", default)]
    pub default_entry: Option<String>,
    /// The timeout of the boot menu.
    #[serde(rename = "menu-timeout", default = "default_menu_timeout")]
    pub menu_timeout: u64,
    /// The style of boot menu to display.
    #[serde(rename = "menu-style", default)]
    pub menu_style: MenuStyle,
    /// Enables autoconfiguration of Sprout based on the environment.
    #[serde(default)]
    pub autoconfigure: bool,
}

/// The default configuration, which matches an empty configuration file.
impl Default for RootConfiguration {
    fn default() -> Self {
        Self {
            version: latest_version(),
            options: Default::default(),
            values: Default::default(),
            drivers: Default::default(),
            extractors: Default::default(),
            actions: Default::default(),
            entries: Default::default(),
            generators: Default::default(),
            phases: Default::default(),
        }
    }
}

/// The default options, which match an empty options section.
impl Default for OptionsConfiguration {
    fn default() -> Self {
        Self {
            default_entry: None,
            menu_timeout: default_menu_timeout(),
            menu_style: Default::default(),
            autoconfigure: false,
        }
    }
}

/// The style of boot menu to display.
#[derive(Serialize, Deserialize, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum MenuStyle {
    /// A basic menu that prints the entries and selects them by number.
    #[serde(rename = "basic")]
    Basic,
    /// A simple full-screen menu that selects entries with the arrow keys.
    #[default]
    #[serde(rename = "simple")]
    Simple,
}

/// Get the latest version of the Sprout configuration format.
pub fn latest_version() -> u32 {
    LATEST_VERSION
}

fn default_menu_timeout() -> u64 {
    DEFAULT_MENU_TIMEOUT_SECONDS
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::value::{Error, MapDeserializer};

    /// Deserialize a configuration from an empty map, like an empty configuration file.
    fn empty() -> RootConfiguration {
        let map = MapDeserializer::<_, Error>::new(core::iter::empty::<(&str, &str)>());
        RootConfiguration::deserialize(map).expect("empty configuration should deserialize")
    }

    #[test]
    fn default_matches_empty_configuration() {
        let default = RootConfiguration::default();
        let empty = empty();
        assert_eq!(default.version, empty.version);
        assert_eq!(default.options.menu_timeout, empty.options.menu_timeout);
        assert_eq!(default.options.menu_style, empty.options.menu_style);
        assert_eq!(default.options.default_entry, empty.options.default_entry);
        assert_eq!(default.options.autoconfigure, empty.options.autoconfigure);
    }

    #[test]
    fn default_menu_timeout_is_not_zero() {
        assert_eq!(
            RootConfiguration::default().options.menu_timeout,
            DEFAULT_MENU_TIMEOUT_SECONDS
        );
    }
}

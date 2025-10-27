use crate::actions::ActionDeclaration;
use crate::drivers::DriverDeclaration;
use crate::entries::EntryDeclaration;
use crate::extractors::ExtractorDeclaration;
use crate::generators::GeneratorDeclaration;
use crate::phases::PhasesConfiguration;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The configuration loader mechanisms.
pub mod loader;

/// This is the latest version of the sprout configuration format.
/// This must be incremented when the configuration breaks compatibility.
pub const LATEST_VERSION: u32 = 1;

/// The default timeout for the boot menu in seconds.
pub const DEFAULT_MENU_TIMEOUT_SECONDS: u64 = 10;

/// The Sprout configuration format.
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct RootConfiguration {
    /// The version of the configuration. This should always be declared
    /// and be the latest version that is supported. If not specified, it is assumed
    /// the configuration is the latest version.
    #[serde(default = "latest_version")]
    pub version: u32,
    /// Default options for Sprout.
    #[serde(default)]
    pub defaults: DefaultsConfiguration,
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

/// Default configuration for Sprout, used when the corresponding options are not specified.
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct DefaultsConfiguration {
    /// The entry to boot without showing the boot menu.
    /// If not specified, a boot menu is shown.
    pub entry: Option<String>,
    /// The timeout of the boot menu.
    #[serde(rename = "menu-timeout", default = "default_menu_timeout")]
    pub menu_timeout: u64,
}

fn latest_version() -> u32 {
    LATEST_VERSION
}

fn default_menu_timeout() -> u64 {
    DEFAULT_MENU_TIMEOUT_SECONDS
}

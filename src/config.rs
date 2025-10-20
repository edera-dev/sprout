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

/// The Sprout configuration format.
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct RootConfiguration {
    /// The version of the configuration. This should always be declared
    /// and be the latest version that is supported. If not specified, it is assumed
    /// the configuration is the latest version.
    #[serde(default = "latest_version")]
    pub version: u32,
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
    #[serde(default)]
    pub actions: BTreeMap<String, ActionDeclaration>,
    #[serde(default)]
    pub entries: BTreeMap<String, EntryDeclaration>,
    #[serde(default)]
    pub generators: BTreeMap<String, GeneratorDeclaration>,
    #[serde(default)]
    pub phases: PhasesConfiguration,
}

fn latest_version() -> u32 {
    LATEST_VERSION
}

use alloc::string::String;
use serde::{Deserialize, Serialize};

/// Declares a driver configuration.
/// Drivers allow extending the functionality of Sprout.
/// Drivers are loaded at runtime and can provide extra functionality like filesystem support.
/// Drivers are loaded by their name, which is used to reference them in other concepts.
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct DriverDeclaration {
    /// The filesystem path to the driver.
    /// This file should be an EFI executable that can be located and executed.
    pub path: String,
    /// What the driver provides.
    /// This decides which controllers are reconnected after the drivers are loaded.
    #[serde(default)]
    pub provides: DriverProvides,
}

/// What a driver provides to the firmware.
#[derive(Serialize, Deserialize, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum DriverProvides {
    /// The driver can provide anything, so every controller is reconnected.
    #[default]
    #[serde(rename = "any")]
    Any,
    /// The driver only provides filesystem or disk support,
    /// so only the controllers that are disks or partitions are reconnected.
    #[serde(rename = "filesystems")]
    Filesystems,
}

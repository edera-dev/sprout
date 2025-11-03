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
}

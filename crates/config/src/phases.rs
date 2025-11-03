use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Configures the various phases of the boot process.
/// This allows hooking various phases to run actions.
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct PhasesConfiguration {
    /// The early phase is run before drivers are loaded.
    #[serde(default)]
    pub early: Vec<PhaseConfiguration>,
    /// The startup phase is run after drivers are loaded, but before entries are displayed.
    #[serde(default)]
    pub startup: Vec<PhaseConfiguration>,
    /// The late phase is run after the entry is chosen, but before the actions are executed.
    #[serde(default)]
    pub late: Vec<PhaseConfiguration>,
}

/// Configures a single phase of the boot process.
/// There can be multiple phase configurations that are
/// executed sequentially.
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct PhaseConfiguration {
    /// The actions to run when the phase is executed.
    #[serde(default)]
    pub actions: Vec<String>,
    /// The values to insert into the context when the phase is executed.
    #[serde(default)]
    pub values: BTreeMap<String, String>,
}

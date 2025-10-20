use crate::actions;
use crate::context::SproutContext;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::rc::Rc;

/// Configures the various phases of the boot process.
/// This allows hooking various phases to run actions.
#[derive(Serialize, Deserialize, Default, Clone)]
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
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct PhaseConfiguration {
    /// The actions to run when the phase is executed.
    #[serde(default)]
    pub actions: Vec<String>,
    /// The values to insert into the context when the phase is executed.
    #[serde(default)]
    pub values: BTreeMap<String, String>,
}

/// Executes the specified [phase] of the boot process.
/// The value [phase] should be a reference of a specific phase in the [PhasesConfiguration].
/// Any error from the actions is propagated into the [Result] and will interrupt further
/// execution of phase actions.
pub fn phase(context: Rc<SproutContext>, phase: &[PhaseConfiguration]) -> Result<()> {
    for item in phase {
        let mut context = context.fork();
        // Insert the values into the context.
        context.insert(&item.values);
        let context = context.freeze();

        // Execute all the actions in this phase configuration.
        for action in item.actions.iter() {
            actions::execute(context.clone(), action)
                .context(format!("unable to execute action '{}'", action))?;
        }
    }
    Ok(())
}

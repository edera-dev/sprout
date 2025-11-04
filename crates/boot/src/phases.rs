use crate::actions;
use crate::context::SproutContext;
use alloc::format;
use alloc::rc::Rc;
use anyhow::{Context, Result};
use edera_sprout_config::phases::PhaseConfiguration;

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

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

/// Manual hook called by code in the bootloader that hands off to another image.
/// This is used to perform actions like clearing the screen.
pub fn before_handoff(context: &SproutContext) -> Result<()> {
    // If we have not been asked to retain the boot console, then we should clear the screen.
    if !context.root().options().retain_boot_console {
        // Clear the screen. We use clear here instead of reset because some firmware,
        // particularly Dell firmware, does not clear the screen on reset.
        // We clear both stdout and stderr because it's not guaranteed that they are the same
        // text output.
        uefi::system::with_stdout(|stdout| stdout.clear()).context("unable to clear screen")?;
        uefi::system::with_stderr(|stderr| stderr.clear()).context("unable to clear screen")?;
    }
    Ok(())
}

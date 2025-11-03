use crate::context::SproutContext;
use alloc::rc::Rc;
use anyhow::{Context, Result, bail};

/// EFI chainloader action.
pub mod chainload;
/// Edera hypervisor action.
pub mod edera;
/// EFI console print action.
pub mod print;

/// Execute the action specified by `name` which should be stored in the
/// root context of the provided `context`. This function may not return
/// if the provided action executes an operating system or an EFI application
/// that does not return control to sprout.
pub fn execute(context: Rc<SproutContext>, name: impl AsRef<str>) -> Result<()> {
    // Retrieve the action from the root context.
    let Some(action) = context.root().actions().get(name.as_ref()) else {
        bail!("unknown action '{}'", name.as_ref());
    };
    // Finalize the context and freeze it.
    let context = context
        .finalize()
        .context("unable to finalize context")?
        .freeze();

    // Execute the action.
    if let Some(chainload) = &action.chainload {
        chainload::chainload(context.clone(), chainload)?;
        return Ok(());
    } else if let Some(print) = &action.print {
        print::print(context.clone(), print)?;
        return Ok(());
    } else if let Some(edera) = &action.edera {
        edera::edera(context.clone(), edera)?;
        return Ok(());
    }

    // If we reach here, we don't know how to execute the action that was configured.
    // This is likely unreachable, but we should still return an error just in case.
    bail!("unknown action configuration");
}

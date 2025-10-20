use crate::context::SproutContext;
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::rc::Rc;

/// EFI chainloader action.
pub mod chainload;
/// Edera hypervisor action.
pub mod edera;
/// EFI console print action.
pub mod print;

/// Splash screen action.
#[cfg(feature = "splash")]
pub mod splash;

/// Declares an action that sprout can execute.
/// Actions allow configuring sprout's internal runtime mechanisms with values
/// that you can specify via other concepts.
///
/// Actions are the main work that Sprout gets done, like booting Linux.
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct ActionDeclaration {
    /// Chainload to another EFI application.
    /// This allows you to load any EFI application, either to boot an operating system
    /// or to perform more EFI actions and return to sprout.
    #[serde(default)]
    pub chainload: Option<chainload::ChainloadConfiguration>,
    /// Print a string to the EFI console.
    #[serde(default)]
    pub print: Option<print::PrintConfiguration>,
    /// Show an image as a fullscreen splash screen.
    #[serde(default)]
    #[cfg(feature = "splash")]
    pub splash: Option<splash::SplashConfiguration>,
    /// Boot the Edera hypervisor and the root operating system.
    /// This action is an extension on top of the Xen EFI stub that
    /// is specific to Edera.
    #[serde(default, rename = "edera")]
    pub edera: Option<edera::EderaConfiguration>,
}

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
    let context = context.finalize().freeze();

    // Execute the action.
    if let Some(chainload) = &action.chainload {
        chainload::chainload(context.clone(), chainload)?;
        return Ok(());
    } else if let Some(print) = &action.print {
        print::print(context.clone(), print)?;
        return Ok(());
    } else if let Some(edera) = &action.edera {
        edera::edera(context.clone(), edera)?;
    }

    #[cfg(feature = "splash")]
    if let Some(splash) = &action.splash {
        splash::splash(context.clone(), splash)?;
        return Ok(());
    }

    // If we reach here, we don't know how to execute the action that was configured.
    // This is likely unreachable, but we should still return an error just in case.
    bail!("unknown action configuration");
}

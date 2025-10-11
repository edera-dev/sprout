use crate::context::SproutContext;
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::rc::Rc;

pub mod chainload;
pub mod print;

#[cfg(feature = "splash")]
pub mod splash;

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct ActionDeclaration {
    #[serde(default)]
    pub chainload: Option<chainload::ChainloadConfiguration>,
    #[serde(default)]
    pub print: Option<print::PrintConfiguration>,
    #[serde(default)]
    #[cfg(feature = "splash")]
    pub splash: Option<splash::SplashConfiguration>,
}

pub fn execute(context: Rc<SproutContext>, name: impl AsRef<str>) -> Result<()> {
    let Some(action) = context.root().actions().get(name.as_ref()) else {
        bail!("unknown action '{}'", name.as_ref());
    };
    let context = context.finalize().freeze();

    if let Some(chainload) = &action.chainload {
        chainload::chainload(context.clone(), chainload)?;
        return Ok(());
    } else if let Some(print) = &action.print {
        print::print(context.clone(), print)?;
        return Ok(());
    }

    #[cfg(feature = "splash")]
    if let Some(splash) = &action.splash {
        splash::splash(context.clone(), splash)?;
        return Ok(());
    }

    bail!("unknown action configuration");
}

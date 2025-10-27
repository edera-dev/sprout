use crate::context::SproutContext;
use anyhow::Result;
use log::info;
use serde::{Deserialize, Serialize};
use std::rc::Rc;

/// The configuration of the print action.
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct PrintConfiguration {
    /// The text to print to the console.
    #[serde(default)]
    pub text: String,
}

/// Executes the print action with the specified `configuration` inside the provided `context`.
pub fn print(context: Rc<SproutContext>, configuration: &PrintConfiguration) -> Result<()> {
    info!("{}", context.stamp(&configuration.text));
    Ok(())
}

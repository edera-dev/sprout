use crate::context::SproutContext;
use anyhow::Result;
use edera_sprout_config::actions::print::PrintConfiguration;
use log::info;
use std::rc::Rc;

/// Executes the print action with the specified `configuration` inside the provided `context`.
pub fn print(context: Rc<SproutContext>, configuration: &PrintConfiguration) -> Result<()> {
    info!("{}", context.stamp(&configuration.text));
    Ok(())
}

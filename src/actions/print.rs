use crate::context::SproutContext;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::rc::Rc;

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct PrintConfiguration {
    #[serde(default)]
    pub text: String,
}

pub fn print(context: Rc<SproutContext>, configuration: &PrintConfiguration) -> Result<()> {
    println!("{}", context.stamp(&configuration.text));
    Ok(())
}

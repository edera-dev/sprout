use crate::context::Context;
use serde::{Deserialize, Serialize};
use std::rc::Rc;

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct PrintConfiguration {
    #[serde(default)]
    pub text: String,
}

pub fn print(context: Rc<Context>, configuration: &PrintConfiguration) {
    println!("{}", context.stamp(&configuration.text));
}

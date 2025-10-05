use crate::config::PrintConfiguration;
use crate::context::Context;
use std::rc::Rc;

pub fn print(context: Rc<Context>, configuration: &PrintConfiguration) {
    println!("{}", context.stamp(&configuration.text));
}

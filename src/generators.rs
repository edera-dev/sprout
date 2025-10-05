use crate::config::{EntryDeclaration, GeneratorDeclaration};
use crate::context::Context;
use std::rc::Rc;

pub mod matrix;

pub fn generate(
    context: Rc<Context>,
    generator: &GeneratorDeclaration,
) -> Vec<(Rc<Context>, EntryDeclaration)> {
    if let Some(matrix) = &generator.matrix {
        matrix::generate(context, matrix)
    } else {
        panic!("unknown action configuration");
    }
}

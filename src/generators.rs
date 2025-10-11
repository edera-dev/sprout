use crate::config::EntryDeclaration;
use crate::context::Context;
use crate::generators::matrix::MatrixConfiguration;
use serde::{Deserialize, Serialize};
use std::rc::Rc;

pub mod matrix;

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct GeneratorDeclaration {
    #[serde(default)]
    pub matrix: Option<MatrixConfiguration>,
}

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

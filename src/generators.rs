use crate::config::EntryDeclaration;
use crate::context::SproutContext;
use crate::generators::matrix::MatrixConfiguration;
use anyhow::Result;
use anyhow::bail;
use serde::{Deserialize, Serialize};
use std::rc::Rc;

pub mod matrix;

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct GeneratorDeclaration {
    #[serde(default)]
    pub matrix: Option<MatrixConfiguration>,
}

pub fn generate(
    context: Rc<SproutContext>,
    generator: &GeneratorDeclaration,
) -> Result<Vec<(Rc<SproutContext>, EntryDeclaration)>> {
    if let Some(matrix) = &generator.matrix {
        matrix::generate(context, matrix)
    } else {
        bail!("unknown action configuration");
    }
}

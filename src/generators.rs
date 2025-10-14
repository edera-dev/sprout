use crate::context::SproutContext;
use crate::entries::EntryDeclaration;
use crate::generators::bls::BlsConfiguration;
use crate::generators::matrix::MatrixConfiguration;
use anyhow::Result;
use anyhow::bail;
use serde::{Deserialize, Serialize};
use std::rc::Rc;

pub mod bls;
pub mod matrix;

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct GeneratorDeclaration {
    #[serde(default)]
    pub matrix: Option<MatrixConfiguration>,
    #[serde(default)]
    pub bls: Option<BlsConfiguration>,
}

pub fn generate(
    context: Rc<SproutContext>,
    generator: &GeneratorDeclaration,
) -> Result<Vec<(Rc<SproutContext>, EntryDeclaration)>> {
    if let Some(matrix) = &generator.matrix {
        matrix::generate(context, matrix)
    } else if let Some(bls) = &generator.bls {
        bls::generate(context, bls)
    } else {
        bail!("unknown generator configuration");
    }
}

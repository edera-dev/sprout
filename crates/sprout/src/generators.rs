use crate::context::SproutContext;
use crate::entries::BootableEntry;
use anyhow::Result;
use anyhow::bail;
use edera_sprout_config::generators::GeneratorDeclaration;
use std::rc::Rc;

/// The BLS generator.
pub mod bls;

/// The list generator.
pub mod list;

/// The matrix generator.
pub mod matrix;

/// Runs the generator specified by the `generator` option.
/// It uses the specified `context` as the parent context for
/// the generated entries, injecting more values if needed.
pub fn generate(
    context: Rc<SproutContext>,
    generator: &GeneratorDeclaration,
) -> Result<Vec<BootableEntry>> {
    if let Some(matrix) = &generator.matrix {
        matrix::generate(context, matrix)
    } else if let Some(bls) = &generator.bls {
        bls::generate(context, bls)
    } else if let Some(list) = &generator.list {
        list::generate(context, list)
    } else {
        bail!("unknown generator configuration");
    }
}

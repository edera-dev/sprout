use crate::context::SproutContext;
use crate::entries::BootableEntry;
use crate::generators::bls::BlsConfiguration;
use crate::generators::list::ListConfiguration;
use crate::generators::matrix::MatrixConfiguration;
use anyhow::Result;
use anyhow::bail;
use serde::{Deserialize, Serialize};
use std::rc::Rc;

pub mod bls;
pub mod list;
pub mod matrix;

/// Declares a generator configuration.
/// Generators allow generating entries at runtime based on a set of data.
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct GeneratorDeclaration {
    /// Matrix generator configuration.
    /// Matrix allows you to specify multiple value-key values as arrays.
    /// This allows multiplying the number of entries by any number of possible
    /// configuration options. For example,
    /// data.x = ["a", "b"]
    /// data.y = ["c", "d"]
    /// would generate an entry for each of these combinations:
    /// x = a, y = c
    /// x = a, y = d
    /// x = b, y = c
    /// x = b, y = d
    #[serde(default)]
    pub matrix: Option<MatrixConfiguration>,
    /// BLS generator configuration.
    /// BLS allows you to pass a filesystem path that contains a set of BLS entries.
    /// It will generate a sprout entry for every supported BLS entry.
    #[serde(default)]
    pub bls: Option<BlsConfiguration>,
    /// List generator configuration.
    /// Allows you to specify a list of values to generate an entry from.
    pub list: Option<ListConfiguration>,
}

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

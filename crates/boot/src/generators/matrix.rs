use crate::context::SproutContext;
use crate::entries::BootableEntry;
use crate::generators::list;
use alloc::rc::Rc;
use alloc::vec::Vec;
use anyhow::Result;
use edera_sprout_config::generators::list::ListConfiguration;
use edera_sprout_config::generators::matrix::MatrixConfiguration;
use edera_sprout_parsing::build_matrix;

/// Generates a set of entries using the specified `matrix` configuration in the `context`.
pub fn generate(
    context: Rc<SproutContext>,
    matrix: &MatrixConfiguration,
) -> Result<Vec<BootableEntry>> {
    // Produce all the combinations of the input values.
    let combinations = build_matrix(&matrix.values);
    // Use the list generator to generate entries for each combination.
    list::generate(
        context,
        &ListConfiguration {
            entry: matrix.entry.clone(),
            values: combinations,
        },
    )
}

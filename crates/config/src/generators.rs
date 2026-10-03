use crate::generators::bls::BlsConfiguration;
use crate::generators::list::ListConfiguration;
use crate::generators::matrix::MatrixConfiguration;
use crate::generators::variants::VariantChoice;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

/// Configuration for the BLS generator.
pub mod bls;

/// Configuration for the list generator.
pub mod list;

/// Configuration for the matrix generator.
pub mod matrix;

/// Configuration for generator variants.
pub mod variants;

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
    /// Variants to apply to every entry produced by the generator.
    /// Each key is an axis, and each axis has a list of choices. Every generated entry
    /// is multiplied by every combination of choices, one choice from each axis.
    /// For example,
    /// variants.console = [{ name = "serial" }, { name = "graphics" }]
    /// variants.mode = [{ name = "normal" }, { name = "debug", values.mode-options = "loglevel=7" }]
    /// would turn each generated entry into four entries:
    /// console = serial, mode = normal
    /// console = serial, mode = debug
    /// console = graphics, mode = normal
    /// console = graphics, mode = debug
    /// The axis name is set to the name of the chosen choice, and then the values of the
    /// chosen choice are inserted, so `$console` and `$mode-options` can be used by the entry.
    /// The entry name is suffixed with the choice names, like `-graphics-debug`.
    /// Axes are applied in alphabetical order. Choices are applied in the declared order.
    #[serde(default)]
    pub variants: BTreeMap<String, Vec<VariantChoice>>,
    /// Rules that exclude generated entries, applied after variants.
    /// Each rule is a map of value names to glob patterns, where `*` matches anything.
    /// An entry is excluded if every value named in any one rule matches its pattern.
    /// Values are matched before they are stamped. For example,
    /// exclude = [{ version = "0-rescue-*" }, { console = "graphics", mode = "debug" }]
    /// excludes BLS rescue entries and the combination of the graphics and debug variants.
    #[serde(default)]
    pub exclude: Vec<BTreeMap<String, String>>,
}

use alloc::string::String;
use serde::{Deserialize, Serialize};

/// The configuration of the print action.
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct PrintConfiguration {
    /// The text to print to the console.
    #[serde(default)]
    pub text: String,
}

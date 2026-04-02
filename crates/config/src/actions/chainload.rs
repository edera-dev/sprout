use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Deserializer, Serialize};

// Naming the return type vastly improves readability
type DeResult<'de, D, T> = core::result::Result<T, <D as Deserializer<'de>>::Error>;

// Type of vector for a list of strings
type StringList = Vec<String>;

// Helper function to allow "linux-initrd" to be either a single string
// or a list of strings in the TOML file.
fn string_or_vec<'de, D>(deserializer: D) -> DeResult<'de, D, StringList>
where
    D: Deserializer<'de>, {

        #[derive(Deserialize)]
        #[serde(untagged)]
        enum StringOrVec {
            String(String),
            Vec(Vec<String>),
        }

        match StringOrVec::deserialize(deserializer)? {
            StringOrVec::String(s) => Ok(alloc::vec![s]),
            // Wrap the vector in Ok() to match the return type
            StringOrVec::Vec(v) => Ok(v),
        }
    }

/// The configuration of the chainload action.
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct ChainloadConfiguration {
    /// The path to the image to chainload.
    /// This can be a Linux EFI stub (vmlinuz usually) or a standard EFI executable.
    pub path: String,

    /// The options to pass to the image.
    /// The options are concatenated by a space and then passed to the EFI application.
    #[serde(default)]
    pub options: Vec<String>,

    /// The path(s) to the initrd to use for the image.
    /// Supports both a single string or an array of strings.
    #[serde(default, rename = "linux-initrd", deserialize_with = "string_or_vec")]
    pub initrd: Vec<String>,
}

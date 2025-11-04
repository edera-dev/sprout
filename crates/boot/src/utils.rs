use alloc::string::{String, ToString};
use alloc::vec::Vec;
use sha2::{Digest, Sha256};

/// Implements a version comparison algorithm according to the BLS specification.
pub mod vercmp;

/// Combine a sequence of strings into a single string, separated by spaces, ignoring empty strings.
pub fn combine_options<T: AsRef<str>>(options: impl Iterator<Item = T>) -> String {
    options
        .flat_map(|item| empty_is_none(Some(item)))
        .map(|item| item.as_ref().to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Produce a unique hash for the input.
/// This uses SHA-256, which is unique enough but relatively short.
pub fn unique_hash(input: &str) -> String {
    hex::encode(Sha256::digest(input.as_bytes()))
}

/// Filter a string-like Option `input` such that an empty string is [None].
pub fn empty_is_none<T: AsRef<str>>(input: Option<T>) -> Option<T> {
    input.filter(|input| !input.as_ref().is_empty())
}

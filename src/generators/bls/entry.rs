use anyhow::{Error, Result};
use std::str::FromStr;

/// Represents a parsed BLS entry.
/// Fields unrelated to Sprout are not included.
#[derive(Default, Debug, Clone)]
pub struct BlsEntry {
    /// The title of the entry.
    pub title: Option<String>,
    /// The options to pass to the entry.
    pub options: Option<String>,
    /// The path to the linux kernel.
    pub linux: Option<String>,
    /// The path to the initrd.
    pub initrd: Option<String>,
    /// The path to an EFI image.
    pub efi: Option<String>,
}

/// Parser for a BLS entry.
impl FromStr for BlsEntry {
    type Err = Error;

    /// Parses the `input` as a BLS entry file.
    fn from_str(input: &str) -> Result<Self> {
        // All the fields in a BLS entry we understand.
        // Set all to None initially.
        let mut title: Option<String> = None;
        let mut options: Option<String> = None;
        let mut linux: Option<String> = None;
        let mut initrd: Option<String> = None;
        let mut efi: Option<String> = None;

        // Iterate over each line in the input and parse it.
        for line in input.lines() {
            // Trim the line.
            let line = line.trim();

            // Split the line once by whitespace.
            let Some((key, value)) = line.split_once(char::is_whitespace) else {
                continue;
            };

            // Match the key to a field we understand.
            match key {
                // The title of the entry.
                "title" => {
                    title = Some(value.trim().to_string());
                }

                // The options to pass to the entry.
                "options" => {
                    options = Some(value.trim().to_string());
                }

                // The path to the linux kernel.
                "linux" => {
                    linux = Some(value.trim().to_string());
                }

                // The path to the initrd.
                "initrd" => {
                    initrd = Some(value.trim().to_string());
                }

                // The path to an EFI image.
                "efi" => {
                    efi = Some(value.trim().to_string());
                }

                // Ignore any other key.
                _ => {
                    continue;
                }
            }
        }

        // Produce a BLS entry from the parsed fields.
        Ok(Self {
            title,
            options,
            linux,
            initrd,
            efi,
        })
    }
}

impl BlsEntry {
    /// Checks if this BLS entry is something we can actually boot in Sprout.
    pub fn is_valid(&self) -> bool {
        self.linux.is_some() || self.efi.is_some()
    }

    /// Fetches the path to an EFI bootable image to boot, if any.
    /// This prioritizes the linux field over efi.
    /// It also converts / to \\ to match EFI path style.
    pub fn chainload_path(&self) -> Option<String> {
        self.linux
            .clone()
            .or(self.efi.clone())
            .map(|path| path.replace("/", "\\").trim_start_matches("\\").to_string())
    }

    /// Fetches the path to an initrd to pass to the kernel, if any.
    /// It also converts / to \\ to match EFI path style.
    pub fn initrd_path(&self) -> Option<String> {
        self.initrd
            .clone()
            .map(|path| path.replace("/", "\\").trim_start_matches("\\").to_string())
    }

    /// Fetches the options to pass to the kernel, if any.
    pub fn options(&self) -> Option<String> {
        self.options.clone()
    }

    /// Fetches the title of the entry, if any.
    pub fn title(&self) -> Option<String> {
        self.title.clone()
    }
}

use crate::extractors::filesystem_device_match::FilesystemDeviceMatchExtractor;
use serde::{Deserialize, Serialize};

/// Configuration for the filesystem-device-match extractor.
pub mod filesystem_device_match;

/// Declares an extractor configuration.
/// Extractors allow calculating values at runtime
/// using built-in sprout modules.
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct ExtractorDeclaration {
    /// The filesystem device match extractor.
    /// This extractor finds a filesystem using some search criteria and returns
    /// the device root path that can concatenated with subpaths to access files
    /// on a particular filesystem.
    #[serde(default, rename = "filesystem-device-match")]
    pub filesystem_device_match: Option<FilesystemDeviceMatchExtractor>,
}

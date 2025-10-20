use crate::context::SproutContext;
use crate::extractors::filesystem_device_match::FilesystemDeviceMatchExtractor;
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::rc::Rc;

/// The filesystem device match extractor.
pub mod filesystem_device_match;

/// Declares an extractor configuration.
/// Extractors allow calculating values at runtime
/// using built-in sprout modules.
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct ExtractorDeclaration {
    /// The filesystem device match extractor.
    /// This extractor finds a filesystem using some search criteria and returns
    /// the device root path that can concatenated with subpaths to access files
    /// on a particular filesystem.
    #[serde(default, rename = "filesystem-device-match")]
    pub filesystem_device_match: Option<FilesystemDeviceMatchExtractor>,
}

/// Extracts the value using the specified `extractor` under the provided `context`.
/// The extractor must return a value, and if a value cannot be determined, an error
/// should be returned.
pub fn extract(context: Rc<SproutContext>, extractor: &ExtractorDeclaration) -> Result<String> {
    if let Some(filesystem) = &extractor.filesystem_device_match {
        filesystem_device_match::extract(context, filesystem)
    } else {
        bail!("unknown extractor configuration");
    }
}

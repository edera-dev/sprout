use crate::context::SproutContext;
use anyhow::{Result, bail};
use edera_sprout_config::extractors::ExtractorDeclaration;
use std::rc::Rc;

/// The filesystem device match extractor.
pub mod filesystem_device_match;

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

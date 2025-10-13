use crate::context::SproutContext;
use crate::extractors::filesystem_device_match::FilesystemDeviceMatchExtractor;
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::rc::Rc;

pub mod filesystem_device_match;

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct ExtractorDeclaration {
    #[serde(default, rename = "filesystem-device-match")]
    pub filesystem_device_match: Option<FilesystemDeviceMatchExtractor>,
}

pub fn extract(context: Rc<SproutContext>, extractor: &ExtractorDeclaration) -> Result<String> {
    if let Some(filesystem) = &extractor.filesystem_device_match {
        filesystem_device_match::extract(context, filesystem)
    } else {
        bail!("unknown extractor configuration");
    }
}

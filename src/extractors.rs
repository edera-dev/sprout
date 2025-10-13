use crate::context::SproutContext;
use crate::extractors::filesystem::FileSystemExtractorConfiguration;
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::rc::Rc;

pub mod filesystem;

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct ExtractorDeclaration {
    pub filesystem: Option<FileSystemExtractorConfiguration>,
}

pub fn extract(context: Rc<SproutContext>, extractor: &ExtractorDeclaration) -> Result<String> {
    if let Some(filesystem) = &extractor.filesystem {
        filesystem::extract(context, filesystem)
    } else {
        bail!("unknown extractor configuration");
    }
}

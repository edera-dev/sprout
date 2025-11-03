use serde::{Deserialize, Serialize};

/// The filesystem device match extractor.
/// This extractor finds a filesystem using some search criteria and returns
/// the device root path that can concatenated with subpaths to access files
/// on a particular filesystem.
/// The fallback value can be used to provide a value if no match is found.
///
/// This extractor requires all the criteria to match. If no criteria is provided,
/// an error is returned.
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct FilesystemDeviceMatchExtractor {
    /// Matches a filesystem that has the specified label.
    #[serde(default, rename = "has-label")]
    pub has_label: Option<String>,
    /// Matches a filesystem that has the specified item.
    /// An item is either a directory or file.
    #[serde(default, rename = "has-item")]
    pub has_item: Option<String>,
    /// Matches a filesystem that has the specified partition UUID.
    #[serde(default, rename = "has-partition-uuid")]
    pub has_partition_uuid: Option<String>,
    /// Matches a filesystem that has the specified partition type UUID.
    #[serde(default, rename = "has-partition-type-uuid")]
    pub has_partition_type_uuid: Option<String>,
    /// The fallback value to use if no filesystem matches the criteria.
    #[serde(default)]
    pub fallback: Option<String>,
}

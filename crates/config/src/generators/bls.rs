use crate::entries::EntryDeclaration;
use alloc::format;
use alloc::string::{String, ToString};
use serde::{Deserialize, Serialize};

/// The default path to the BLS directory.
const BLS_TEMPLATE_PATH: &str = "\\loader";

/// The directory of unified kernel images, relative to the root of the filesystem.
const BLS_UKI_DIRECTORY: &str = "\\EFI\\Linux";

/// The configuration of the BLS generator.
/// The BLS uses the Bootloader Specification to produce
/// entries from an input template.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlsConfiguration {
    /// The entry to use for as a template.
    pub entry: EntryDeclaration,
    /// The path to the BLS directory.
    #[serde(default = "default_bls_path")]
    pub path: String,
    /// The path to the directory of unified kernel images, which are BLS Type #2 entries.
    /// Every `.efi` file in the directory becomes an entry. When this is not set, the
    /// directory is `\EFI\Linux` on the same device as `path`. An empty path disables
    /// unified kernel images. Two generators that read the same directory produce entries
    /// with the same names, so they should not both scan it.
    #[serde(default, rename = "uki-path")]
    pub uki_path: Option<String>,
    /// Whether generated entries keep the name of the BLS entry file as-is.
    /// When enabled, the generator name is not prepended, so the entry names match
    /// the BLS entry ids used by tools like `bootctl set-default`. Variants still append
    /// their choice names, so entries of a generator with variants never match the ids
    /// exactly, and are best selected with a pattern like `<id>-*`. Two BLS generators
    /// that read the same BLS directory will produce entries with the same names, so
    /// generators that reuse a BLS directory should disable this.
    #[serde(default = "default_pin_names", rename = "pin-names")]
    pub pin_names: bool,
}

impl BlsConfiguration {
    /// The directory of unified kernel images for the stamped BLS path `bls_path`,
    /// or None if unified kernel images are disabled.
    pub fn uki_path_for(&self, bls_path: &str) -> Option<String> {
        match self.uki_path.as_deref() {
            Some("") => None,
            Some(path) => Some(path.to_string()),
            // Keep the device of the BLS path, which is everything up to the last slash.
            None => {
                let device = bls_path.rfind('/').map_or("", |index| &bls_path[..=index]);
                Some(format!("{}{}", device, BLS_UKI_DIRECTORY))
            }
        }
    }
}

/// The default configuration, which matches an empty BLS generator section.
impl Default for BlsConfiguration {
    fn default() -> Self {
        Self {
            entry: Default::default(),
            path: default_bls_path(),
            uki_path: None,
            pin_names: default_pin_names(),
        }
    }
}

fn default_pin_names() -> bool {
    true
}

fn default_bls_path() -> String {
    BLS_TEMPLATE_PATH.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uki_path_is_next_to_the_default_bls_path() {
        let bls = BlsConfiguration::default();
        assert_eq!(
            bls.uki_path_for("\\loader").as_deref(),
            Some("\\EFI\\Linux")
        );
    }

    #[test]
    fn uki_path_follows_the_device_of_the_bls_path() {
        let bls = BlsConfiguration::default();
        assert_eq!(
            bls.uki_path_for("PciRoot(0x0)/HD(2,GPT,abc)/\\loader")
                .as_deref(),
            Some("PciRoot(0x0)/HD(2,GPT,abc)/\\EFI\\Linux")
        );
    }

    #[test]
    fn explicit_uki_path_is_used_as_is() {
        let bls = BlsConfiguration {
            uki_path: Some("\\uki".to_string()),
            ..Default::default()
        };
        assert_eq!(
            bls.uki_path_for("HD(2,GPT,abc)/\\loader").as_deref(),
            Some("\\uki")
        );
    }

    #[test]
    fn empty_uki_path_disables_unified_kernel_images() {
        let bls = BlsConfiguration {
            uki_path: Some(String::new()),
            ..Default::default()
        };
        assert_eq!(bls.uki_path_for("\\loader"), None);
    }
}

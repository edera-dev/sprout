use crate::BlsEntry;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// The PE sections of a unified kernel image that Sprout reads.
pub const UKI_SECTIONS: [&str; 3] = [".osrel", ".cmdline", ".uname"];

/// A parsed os-release file, such as the `.osrel` section of a unified kernel image.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct OsRelease {
    fields: BTreeMap<String, String>,
}

impl OsRelease {
    /// Parses the `input` as an os-release file, which has one `KEY=value` per line.
    /// Lines that aren't a setting are ignored, and the last value of a key is used.
    pub fn parse(input: &str) -> Self {
        let mut fields = BTreeMap::new();
        for line in input.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            fields.insert(key.trim().to_string(), unquote(value.trim()));
        }
        Self { fields }
    }

    /// Fetches the value of `key`, if it is set.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.fields.get(key).map(String::as_str)
    }
}

/// Removes one pair of matching quotes from `value` and unescapes a double quoted value.
fn unquote(value: &str) -> String {
    if value.len() >= 2 && value.starts_with('\'') && value.ends_with('\'') {
        return value[1..value.len() - 1].to_string();
    }
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        let mut result = String::new();
        let mut chars = value[1..value.len() - 1].chars();
        while let Some(c) = chars.next() {
            // Only these characters can be escaped. Any other backslash is kept as written.
            match (c, chars.clone().next()) {
                ('\\', Some(next @ ('"' | '\\' | '$' | '`'))) => {
                    result.push(next);
                    chars.next();
                }
                _ => result.push(c),
            }
        }
        return result;
    }
    value.to_string()
}

/// Decodes a section as text, dropping the NUL and whitespace padding at the end.
/// An empty section is treated as missing.
fn section_text(sections: &BTreeMap<String, Vec<u8>>, name: &str) -> Option<String> {
    let text = String::from_utf8_lossy(sections.get(name)?);
    let text = text.trim_end_matches(|c: char| c == '\0' || c.is_whitespace());
    (!text.is_empty()).then(|| text.to_string())
}

impl BlsEntry {
    /// Produces an entry for the unified kernel image at `efi_path`, from its PE `sections`.
    /// The title comes from `PRETTY_NAME`, then `ID`. The version comes from `IMAGE_VERSION`,
    /// `VERSION_ID`, `BUILD_ID`, then the `.uname` section. The sort key comes from `IMAGE_ID`,
    /// then `ID`. The embedded command line is kept in `cmdline` and not in `options`, as the
    /// image reads its own command line.
    pub fn from_uki(sections: &BTreeMap<String, Vec<u8>>, efi_path: &str) -> Self {
        let os_release = section_text(sections, ".osrel")
            .map(|text| OsRelease::parse(&text))
            .unwrap_or_default();
        let first = |keys: &[&str]| {
            keys.iter()
                .filter_map(|key| os_release.get(key))
                .find(|value| !value.is_empty())
                .map(ToString::to_string)
        };
        let uname = section_text(sections, ".uname");

        Self {
            title: first(&["PRETTY_NAME", "ID"]),
            version: first(&["IMAGE_VERSION", "VERSION_ID", "BUILD_ID"]).or_else(|| uname.clone()),
            sort_key: first(&["IMAGE_ID", "ID"]),
            efi: Some(efi_path.to_string()),
            cmdline: section_text(sections, ".cmdline"),
            uname,
            ..Self::default()
        }
    }
}

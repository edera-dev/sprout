use crate::{BlsEntry, PeSection};
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// The PE sections of a unified kernel image that Sprout reads.
pub const UKI_SECTIONS: [&str; 4] = [".osrel", ".cmdline", ".uname", ".profile"];

/// The most profiles that are read from a unified kernel image.
pub const MAX_PROFILES: usize = 256;

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
    /// Produces an entry for one profile of the unified kernel image at `uki_path`.
    /// A profile after the first is booted with `@<number>` in its load options.
    pub fn from_uki_profile(profile: &UkiProfile, uki_path: &str) -> Self {
        Self::from_uki_profile_with(profile, uki_path, false)
    }

    /// Like [BlsEntry::from_uki_profile], and `strict` follows systemd-boot exactly.
    pub fn from_uki_profile_with(profile: &UkiProfile, uki_path: &str, strict: bool) -> Self {
        let mut entry = Self::from_uki_with(&profile.sections, uki_path, strict);
        if let Some(index) = profile.index {
            entry.profile = (index > 0).then(|| index.to_string());
            entry.title = entry
                .title
                .take()
                .map(|title| profile_title(&title, index, &profile.info));
        }
        entry
    }

    /// Produces an entry for the unified kernel image at `uki_path`, from its PE `sections`.
    pub fn from_uki(sections: &BTreeMap<String, Vec<u8>>, uki_path: &str) -> Self {
        Self::from_uki_with(sections, uki_path, false)
    }

    /// Produces an entry for the unified kernel image at `uki_path`, from its PE `sections`.
    /// The title comes from `PRETTY_NAME`, `IMAGE_ID`, `NAME`, then `ID`. The version comes from
    /// `IMAGE_VERSION`, `VERSION`, `VERSION_ID`, then `BUILD_ID`, as in systemd-boot, and then
    /// from the `.uname` section unless `strict` is set. The sort key comes from `IMAGE_ID`, then
    /// `ID`. The embedded command line is kept in `cmdline` and not in `options`, as the image
    /// reads its own command line.
    pub fn from_uki_with(
        sections: &BTreeMap<String, Vec<u8>>,
        uki_path: &str,
        strict: bool,
    ) -> Self {
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
        let version = first(&["IMAGE_VERSION", "VERSION", "VERSION_ID", "BUILD_ID"]);

        Self {
            title: first(&["PRETTY_NAME", "IMAGE_ID", "NAME", "ID"]),
            version: version.or_else(|| uname.clone().filter(|_| !strict)),
            sort_key: first(&["IMAGE_ID", "ID"]),
            uki: Some(uki_path.to_string()),
            cmdline: section_text(sections, ".cmdline"),
            uname,
            ..Self::default()
        }
    }
}

/// The metadata in the `.profile` section of a profile of a unified kernel image.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ProfileInfo {
    /// A short identifier of the profile.
    pub id: Option<String>,
    /// A title for people to read.
    pub title: Option<String>,
}

impl ProfileInfo {
    /// Parses the text of a `.profile` section, which is in the format of an os-release file.
    /// A key without a value is treated as missing.
    pub fn parse(text: &str) -> Self {
        let os_release = OsRelease::parse(text);
        let get = |key: &str| {
            os_release
                .get(key)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
        };
        Self {
            id: get("ID"),
            title: get("TITLE"),
        }
    }
}

/// The sections that one profile of a unified kernel image boots with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UkiProfile {
    /// The number of the profile, or None if the image has no `.profile` sections.
    pub index: Option<u32>,
    /// The metadata of the profile.
    pub info: ProfileInfo,
    /// The sections of the profile on top of those of the base image. A section that is empty
    /// or too large to read, which is how a profile removes a section, is not here.
    pub sections: BTreeMap<String, Vec<u8>>,
}

/// Keeps the first section of each name in `range`.
fn first_of_each(range: &[PeSection]) -> BTreeMap<String, Option<Vec<u8>>> {
    let mut sections = BTreeMap::new();
    for section in range {
        sections
            .entry(section.name.clone())
            .or_insert_with(|| section.data.clone());
    }
    sections
}

/// Splits the `sections` of a unified kernel image, in the order of the section table, into the
/// profiles of the image. Each `.profile` section starts a profile that goes on until the next
/// one, and the sections before the first are the base that every profile starts from.
/// A section of a profile takes the place of the section of the same name in the base.
/// An image without `.profile` sections has one profile, which is the base.
pub fn uki_profiles(sections: &[PeSection]) -> Vec<UkiProfile> {
    // Sections that are empty or too large are not available.
    let usable = |map: BTreeMap<String, Option<Vec<u8>>>| -> BTreeMap<String, Vec<u8>> {
        map.into_iter()
            .filter_map(|(name, data)| {
                data.filter(|data| !data.is_empty())
                    .map(|data| (name, data))
            })
            .collect()
    };

    let starts: Vec<usize> = sections
        .iter()
        .enumerate()
        .filter(|(_, section)| section.name == ".profile")
        .map(|(index, _)| index)
        .collect();
    let Some(&first) = starts.first() else {
        return alloc::vec![UkiProfile {
            index: None,
            info: ProfileInfo::default(),
            sections: usable(first_of_each(sections)),
        }];
    };

    let base = first_of_each(&sections[..first]);
    starts
        .iter()
        .take(MAX_PROFILES)
        .enumerate()
        .map(|(index, start)| {
            let end = starts.get(index + 1).copied().unwrap_or(sections.len());
            let range = &sections[*start..end];
            let mut merged = base.clone();
            merged.extend(first_of_each(range));
            UkiProfile {
                index: Some(index as u32),
                info: range[0]
                    .data
                    .as_ref()
                    .map(|data| ProfileInfo::parse(&String::from_utf8_lossy(data)))
                    .unwrap_or_default(),
                sections: usable(merged),
            }
        })
        .collect()
}

/// The part of the id of an entry that comes after the `@`, which names the profile.
/// The first profile has none, and the others have their identifier or their number.
pub fn profile_id_suffix(index: u32, info: &ProfileInfo) -> Option<String> {
    (index > 0).then(|| info.id.clone().unwrap_or_else(|| index.to_string()))
}

/// The title of the entry for a profile, given the `name` of the image.
pub fn profile_title(name: &str, index: u32, info: &ProfileInfo) -> String {
    match (&info.title, &info.id) {
        (Some(title), _) => format!("{} ({})", name, title),
        (None, Some(id)) if index > 0 => format!("{} ({})", name, id),
        (None, None) if index > 0 => format!("{} (Profile #{})", name, index + 1),
        _ => name.to_string(),
    }
}

/// The id of the entry for a profile of the image with the file name `file_id`, such as
/// `fedora.efi`. The file name is in lower case, as in systemd-boot, and the profile is not.
pub fn profile_entry_id(file_id: &str, suffix: Option<&str>) -> String {
    match suffix {
        Some(suffix) => format!("{}@{}", file_id.to_lowercase(), suffix),
        None => file_id.to_lowercase(),
    }
}

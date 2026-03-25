#![no_std]
extern crate alloc;

use alloc::string::{String, ToString};
use anyhow::{Error, Result};
use core::{cmp::Ordering, iter::Peekable, str::FromStr};

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
    /// The sort key for the entry.
    pub sort_key: Option<String>,
    /// The version of the entry.
    pub version: Option<String>,
    /// The machine id of the entry.
    pub machine_id: Option<String>,
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
        let mut sort_key: Option<String> = None;
        let mut version: Option<String> = None;
        let mut machine_id: Option<String> = None;

        // Iterate over each line in the input and parse it.
        for line in input.lines() {
            let line = line.trim();
            // Skip over empty lines and comments.
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Split the line once by whitespace. This technically includes newlines but since
            // the lines iterator is used, there should never be a newline here.
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

                "sort-key" => {
                    sort_key = Some(value.trim().to_string());
                }

                "version" => {
                    version = Some(value.trim().to_string());
                }

                "machine-id" => {
                    machine_id = Some(value.trim().to_string());
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
            sort_key,
            version,
            machine_id,
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
            .map(|path| path.replace('/', "\\").trim_start_matches('\\').to_string())
    }

    /// Fetches the path to an initrd to pass to the kernel, if any.
    /// It also converts / to \\ to match EFI path style.
    pub fn initrd_path(&self) -> Option<String> {
        self.initrd
            .clone()
            .map(|path| path.replace('/', "\\").trim_start_matches('\\').to_string())
    }

    /// Fetches the options to pass to the kernel, if any.
    pub fn options(&self) -> Option<String> {
        self.options.clone()
    }

    /// Fetches the title of the entry, if any.
    pub fn title(&self) -> Option<String> {
        self.title.clone()
    }

    /// Fetches the sort key of the entry, if any.
    pub fn sort_key(&self) -> Option<String> {
        self.sort_key.clone()
    }

    /// Fetches the version of the entry, if any.
    pub fn version(&self) -> Option<String> {
        self.version.clone()
    }

    /// Fetches the machine id of the entry, if any.
    pub fn machine_id(&self) -> Option<String> {
        self.machine_id.clone()
    }
}

/// Sorts two BLS entries according to the BLS sort system.
/// `a_name` and `b_name` are the entry filenames (without `.conf`) used as the
/// final tiebreaker when all other fields are equal.
/// Reference: <https://uapi-group.org/specifications/specs/boot_loader_specification/#sorting>
pub fn sort_bls(a_bls: &BlsEntry, a_name: &str, b_bls: &BlsEntry, b_name: &str) -> Ordering {
    // Grab the sort keys from both entries.
    let a_sort_key = a_bls.sort_key();
    let b_sort_key = b_bls.sort_key();

    // Compare the sort keys of both entries.
    match a_sort_key.cmp(&b_sort_key) {
        // If A and B sort keys are equal, sort by machine-id.
        Ordering::Equal => {
            // Grab the machine-id from both entries.
            let a_machine_id = a_bls.machine_id();
            let b_machine_id = b_bls.machine_id();

            // Compare the machine-id of both entries.
            match a_machine_id.cmp(&b_machine_id) {
                // If both machine-id values are equal, sort by version.
                Ordering::Equal => {
                    // Grab the version from both entries.
                    let a_version = a_bls.version();
                    let b_version = b_bls.version();

                    // Compare the version of both entries, sorting newer versions first.
                    match compare_versions_optional(a_version.as_deref(), b_version.as_deref())
                        .reverse()
                    {
                        // If both versions are equal, sort by file name in reverse order.
                        Ordering::Equal => {
                            // Compare the file names of both entries, sorting newer entries first.
                            compare_versions(a_name, b_name).reverse()
                        }
                        other => other,
                    }
                }
                other => other,
            }
        }

        other => other,
    }
}

/// Handles single character advancement and comparison.
macro_rules! handle_single_char {
    ($ca: expr, $cb:expr, $a_chars:expr, $b_chars:expr, $c:expr) => {
        match ($ca == $c, $cb == $c) {
            (true, false) => return Ordering::Less,
            (false, true) => return Ordering::Greater,
            (true, true) => {
                $a_chars.next();
                $b_chars.next();
                continue;
            }
            _ => {}
        }
    };
}

/// Compares two strings using the BLS version comparison specification.
/// Handles optional values as well by comparing only if both are specified.
pub fn compare_versions_optional(a: Option<&str>, b: Option<&str>) -> Ordering {
    match (a, b) {
        // If both have values, compare them.
        (Some(a), Some(b)) => compare_versions(a, b),
        // If the second value is None, then `a` is less than `b`.
        (Some(_a), None) => Ordering::Less,
        // If the first value is None, the `a` is greater than `b`.
        (None, Some(_b)) => Ordering::Greater,
        // If both values are None, return that they are equal.
        (None, None) => Ordering::Equal,
    }
}

/// Compares two strings using the BLS version comparison specification.
/// See: <https://uapi-group.org/specifications/specs/version_format_specification/>
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    // Acquire a peekable iterator for each string.
    let mut a_chars = a.chars().peekable();
    let mut b_chars = b.chars().peekable();

    // Loop until we have reached the end of one of the strings.
    loop {
        // Skip invalid characters in both strings.
        skip_invalid(&mut a_chars);
        skip_invalid(&mut b_chars);

        // Check if either string has ended.
        match (a_chars.peek(), b_chars.peek()) {
            // No more characters in either string.
            (None, None) => return Ordering::Equal,
            // One string has ended, the other hasn't.
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            // Both strings have characters left.
            (Some(&ca), Some(&cb)) => {
                // Handle the ~ character.
                handle_single_char!(ca, cb, a_chars, b_chars, '~');

                // Handle '-' character.
                handle_single_char!(ca, cb, a_chars, b_chars, '-');

                // Handle the '^' character.
                handle_single_char!(ca, cb, a_chars, b_chars, '^');

                // Handle the '.' character.
                handle_single_char!(ca, cb, a_chars, b_chars, '.');

                // Handle digits with numerical comparison.
                // We key off of the A character being a digit intentionally as we presume
                // this indicates it will be the same at this position.
                if ca.is_ascii_digit() || cb.is_ascii_digit() {
                    let result = compare_numeric(&mut a_chars, &mut b_chars);
                    if result != Ordering::Equal {
                        return result;
                    }
                    continue;
                }

                // Handle letters with alphabetical comparison.
                // We key off of the A character being alphabetical intentionally as we presume
                // this indicates it will be the same at this position.
                if ca.is_ascii_alphabetic() || cb.is_ascii_alphabetic() {
                    let result = compare_alphabetic(&mut a_chars, &mut b_chars);
                    if result != Ordering::Equal {
                        return result;
                    }
                    continue;
                }
            }
        }
    }
}

/// Skips characters that are not in the valid character set.
fn skip_invalid<I: Iterator<Item = char>>(iter: &mut Peekable<I>) {
    while let Some(&c) = iter.peek() {
        if is_valid_char(c) {
            break;
        }
        iter.next();
    }
}

/// Checks if a character is in the valid character set for comparison.
fn is_valid_char(c: char) -> bool {
    matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '.' | '~' | '^')
}

/// Compares numerical prefixes by extracting numbers.
fn compare_numeric<I: Iterator<Item = char>>(
    iter_a: &mut Peekable<I>,
    iter_b: &mut Peekable<I>,
) -> Ordering {
    let num_a = extract_number(iter_a);
    let num_b = extract_number(iter_b);

    num_a.cmp(&num_b)
}

/// Extracts a number from the iterator, skipping leading zeros.
fn extract_number<I: Iterator<Item = char>>(iter: &mut Peekable<I>) -> u64 {
    // Skip leading zeros
    while let Some(&'0') = iter.peek() {
        iter.next();
    }

    let mut num = 0u64;
    while let Some(&c) = iter.peek() {
        if c.is_ascii_digit() {
            iter.next();
            num = num.saturating_mul(10).saturating_add(c as u64 - '0' as u64);
        } else {
            break;
        }
    }

    num
}

/// Compares alphabetical prefixes.
/// Capital letters compare lower than lowercase letters (B < a).
fn compare_alphabetic<I: Iterator<Item = char>>(
    iter_a: &mut Peekable<I>,
    iter_b: &mut Peekable<I>,
) -> Ordering {
    loop {
        return match (iter_a.peek(), iter_b.peek()) {
            (Some(&ca), Some(&cb)) if ca.is_ascii_alphabetic() && cb.is_ascii_alphabetic() => {
                if ca == cb {
                    // Same character, we should continue.
                    iter_a.next();
                    iter_b.next();
                    continue;
                }

                // Different characters found.
                // All capital letters compare lower than lowercase letters.
                match (ca.is_ascii_uppercase(), cb.is_ascii_uppercase()) {
                    (true, false) => Ordering::Less,    // uppercase < lowercase
                    (false, true) => Ordering::Greater, // lowercase > uppercase
                    (true, true) => ca.cmp(&cb),        // both are uppercase
                    (false, false) => ca.cmp(&cb),      // both are lowercase
                }
            }

            (Some(&ca), Some(_)) if ca.is_ascii_alphabetic() => {
                // a has letters, b doesn't
                Ordering::Greater
            }

            (Some(_), Some(&cb)) if cb.is_ascii_alphabetic() => {
                // b has letters, a doesn't
                Ordering::Less
            }

            (Some(&ca), None) if ca.is_ascii_alphabetic() => Ordering::Greater,

            (None, Some(&cb)) if cb.is_ascii_alphabetic() => Ordering::Less,

            _ => Ordering::Equal,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cmp::Ordering;

    fn sort_entry(
        sort_key: Option<&str>,
        machine_id: Option<&str>,
        version: Option<&str>,
    ) -> BlsEntry {
        BlsEntry {
            sort_key: sort_key.map(|s| s.to_string()),
            machine_id: machine_id.map(|s| s.to_string()),
            version: version.map(|s| s.to_string()),
            linux: Some("/vmlinuz".to_string()),
            ..BlsEntry::default()
        }
    }

    #[test]
    fn parse_empty_input_gives_all_none() {
        let entry: BlsEntry = "".parse().unwrap();
        assert!(entry.title.is_none());
        assert!(entry.linux.is_none());
        assert!(entry.efi.is_none());
        assert!(entry.initrd.is_none());
        assert!(entry.options.is_none());
        assert!(entry.sort_key.is_none());
        assert!(entry.version.is_none());
        assert!(entry.machine_id.is_none());
    }

    #[test]
    fn parse_all_known_fields() {
        let input = "\
title      Fedora Linux 6.5.6
version    6.5.6-300.fc39.x86_64
machine-id abc123def456
linux      /boot/vmlinuz-6.5.6
initrd     /boot/initrd-6.5.6.img
options    root=/dev/sda1 ro quiet
sort-key   fedora
efi        /EFI/fedora/shimx64.efi
";
        let entry: BlsEntry = input.parse().unwrap();
        assert_eq!(entry.title.as_deref(), Some("Fedora Linux 6.5.6"));
        assert_eq!(entry.version.as_deref(), Some("6.5.6-300.fc39.x86_64"));
        assert_eq!(entry.machine_id.as_deref(), Some("abc123def456"));
        assert_eq!(entry.linux.as_deref(), Some("/boot/vmlinuz-6.5.6"));
        assert_eq!(entry.initrd.as_deref(), Some("/boot/initrd-6.5.6.img"));
        assert_eq!(entry.options.as_deref(), Some("root=/dev/sda1 ro quiet"));
        assert_eq!(entry.sort_key.as_deref(), Some("fedora"));
        assert_eq!(entry.efi.as_deref(), Some("/EFI/fedora/shimx64.efi"));
    }

    #[test]
    fn parse_skips_blank_lines() {
        let input = "\n\ntitle   My Entry\n\n\nlinux /vmlinuz\n\n";
        let entry: BlsEntry = input.parse().unwrap();
        assert_eq!(entry.title.as_deref(), Some("My Entry"));
        assert_eq!(entry.linux.as_deref(), Some("/vmlinuz"));
    }

    #[test]
    fn parse_skips_comment_lines() {
        let input = "# this is a comment\ntitle My Entry\n# another comment\nlinux /vmlinuz\n";
        let entry: BlsEntry = input.parse().unwrap();
        assert_eq!(entry.title.as_deref(), Some("My Entry"));
        assert_eq!(entry.linux.as_deref(), Some("/vmlinuz"));
    }

    #[test]
    fn parse_trims_leading_whitespace_from_value() {
        let input = "title    Padded Title\nlinux    /boot/vmlinuz\n";
        let entry: BlsEntry = input.parse().unwrap();
        assert_eq!(entry.title.as_deref(), Some("Padded Title"));
        assert_eq!(entry.linux.as_deref(), Some("/boot/vmlinuz"));
    }

    #[test]
    fn parse_ignores_unknown_keys() {
        let input = "title My Entry\nunknown-key some-value\nfuture-field value\nlinux /vmlinuz\n";
        let entry: BlsEntry = input.parse().unwrap();
        assert_eq!(entry.title.as_deref(), Some("My Entry"));
        assert_eq!(entry.linux.as_deref(), Some("/vmlinuz"));
    }

    #[test]
    fn parse_skips_lines_without_whitespace_separator() {
        // A line with no whitespace cannot be split into key+value, so it is skipped
        let input = "title My Entry\nnovalueline\nlinux /vmlinuz\n";
        let entry: BlsEntry = input.parse().unwrap();
        assert_eq!(entry.title.as_deref(), Some("My Entry"));
        assert_eq!(entry.linux.as_deref(), Some("/vmlinuz"));
    }

    #[test]
    fn is_valid_when_linux_present() {
        let entry: BlsEntry = "linux /vmlinuz\n".parse().unwrap();
        assert!(entry.is_valid());
    }

    #[test]
    fn is_valid_when_efi_present() {
        let entry: BlsEntry = "efi /EFI/boot/bootx64.efi\n".parse().unwrap();
        assert!(entry.is_valid());
    }

    #[test]
    fn not_valid_without_linux_or_efi() {
        let entry: BlsEntry = "title Just a Title\noptions quiet\n".parse().unwrap();
        assert!(!entry.is_valid());
    }

    #[test]
    fn chainload_path_normalises_forward_slashes_to_backslashes() {
        let entry: BlsEntry = "linux /boot/vmlinuz\n".parse().unwrap();
        assert_eq!(entry.chainload_path().as_deref(), Some("boot\\vmlinuz"));
    }

    #[test]
    fn chainload_path_strips_leading_backslash() {
        let entry: BlsEntry = "linux \\EFI\\boot\\kernel\n".parse().unwrap();
        assert_eq!(entry.chainload_path().as_deref(), Some("EFI\\boot\\kernel"));
    }

    #[test]
    fn chainload_path_prefers_linux_over_efi() {
        let input = "linux /boot/vmlinuz\nefi /EFI/boot/bootx64.efi\n";
        let entry: BlsEntry = input.parse().unwrap();
        assert_eq!(entry.chainload_path().as_deref(), Some("boot\\vmlinuz"));
    }

    #[test]
    fn chainload_path_falls_back_to_efi_when_no_linux() {
        let entry: BlsEntry = "efi /EFI/Microsoft/Boot/bootmgfw.efi\n".parse().unwrap();
        assert_eq!(
            entry.chainload_path().as_deref(),
            Some("EFI\\Microsoft\\Boot\\bootmgfw.efi")
        );
    }

    #[test]
    fn chainload_path_none_when_neither_linux_nor_efi() {
        let entry: BlsEntry = "title Only Title\n".parse().unwrap();
        assert!(entry.chainload_path().is_none());
    }

    #[test]
    fn initrd_path_normalises_slashes() {
        let entry: BlsEntry = "linux /vmlinuz\ninitrd /boot/initrd.img\n".parse().unwrap();
        assert_eq!(entry.initrd_path().as_deref(), Some("boot\\initrd.img"));
    }

    #[test]
    fn initrd_path_none_when_not_set() {
        let entry: BlsEntry = "linux /vmlinuz\n".parse().unwrap();
        assert!(entry.initrd_path().is_none());
    }

    #[test]
    fn sort_key_is_primary_criterion() {
        let a = sort_entry(Some("alpine"), None, None);
        let b = sort_entry(Some("fedora"), None, None);
        assert_eq!(sort_bls(&a, "a", &b, "b"), Ordering::Less);
        assert_eq!(sort_bls(&b, "b", &a, "a"), Ordering::Greater);
    }

    #[test]
    fn machine_id_is_secondary_criterion() {
        let a = sort_entry(Some("linux"), Some("aaa"), None);
        let b = sort_entry(Some("linux"), Some("bbb"), None);
        assert_eq!(sort_bls(&a, "a", &b, "b"), Ordering::Less);
        assert_eq!(sort_bls(&b, "b", &a, "a"), Ordering::Greater);
    }

    #[test]
    fn version_is_tertiary_criterion_newer_first() {
        let a = sort_entry(Some("linux"), Some("abc"), Some("6.5.0"));
        let b = sort_entry(Some("linux"), Some("abc"), Some("6.4.0"));
        // newer version (a) sorts before older version (b), so a < b in sort order
        assert_eq!(sort_bls(&a, "a", &b, "b"), Ordering::Less);
        assert_eq!(sort_bls(&b, "b", &a, "a"), Ordering::Greater);
    }

    #[test]
    fn name_is_final_tiebreaker_newer_first() {
        let a = sort_entry(Some("linux"), Some("abc"), Some("6.5.0"));
        let b = sort_entry(Some("linux"), Some("abc"), Some("6.5.0"));
        // name comparison via compare_versions, reversed — higher name sorts first
        assert_eq!(sort_bls(&a, "entry-2", &b, "entry-1"), Ordering::Less);
        assert_eq!(sort_bls(&a, "entry-1", &b, "entry-2"), Ordering::Greater);
    }

    #[test]
    fn identical_entries_are_equal() {
        let a = sort_entry(Some("linux"), Some("abc"), Some("6.5.0"));
        let b = sort_entry(Some("linux"), Some("abc"), Some("6.5.0"));
        assert_eq!(sort_bls(&a, "entry-1", &b, "entry-1"), Ordering::Equal);
    }

    #[test]
    fn equal_strings_are_equal() {
        assert_eq!(compare_versions("1.0.0", "1.0.0"), Ordering::Equal);
    }

    #[test]
    fn empty_strings_are_equal() {
        assert_eq!(compare_versions("", ""), Ordering::Equal);
    }

    #[test]
    fn simple_numeric_less() {
        assert_eq!(compare_versions("1", "2"), Ordering::Less);
    }

    #[test]
    fn simple_numeric_greater() {
        assert_eq!(compare_versions("2", "1"), Ordering::Greater);
    }

    #[test]
    fn numeric_is_not_lexicographic() {
        // "10" > "9", not "10" < "9" as in lexicographic order
        assert_eq!(compare_versions("10", "9"), Ordering::Greater);
        assert_eq!(compare_versions("1.10", "1.9"), Ordering::Greater);
    }

    #[test]
    fn leading_zeros_are_ignored() {
        assert_eq!(compare_versions("01", "1"), Ordering::Equal);
        assert_eq!(compare_versions("1.00", "1.0"), Ordering::Equal);
        assert_eq!(compare_versions("007", "7"), Ordering::Equal);
    }

    #[test]
    fn multi_component_comparison() {
        assert_eq!(compare_versions("1.0.0", "1.0.1"), Ordering::Less);
        assert_eq!(compare_versions("1.0.1", "1.0.0"), Ordering::Greater);
        assert_eq!(compare_versions("2.0.0", "1.9.9"), Ordering::Greater);
    }

    #[test]
    fn more_components_is_greater() {
        assert_eq!(compare_versions("1.0.0", "1.0"), Ordering::Greater);
        assert_eq!(compare_versions("1.0", "1.0.0"), Ordering::Less);
        assert_eq!(compare_versions("1.0.0.0", "1.0.0"), Ordering::Greater);
    }

    #[test]
    fn alphabetic_sections_compare_lexicographically() {
        assert_eq!(compare_versions("1.0a", "1.0b"), Ordering::Less);
        assert_eq!(compare_versions("1.0b", "1.0a"), Ordering::Greater);
        assert_eq!(compare_versions("1.0abc", "1.0abd"), Ordering::Less);
    }

    #[test]
    fn uppercase_letters_sort_below_lowercase() {
        // Capital letters compare lower than lowercase (B < a)
        assert_eq!(compare_versions("B", "a"), Ordering::Less);
        assert_eq!(compare_versions("a", "B"), Ordering::Greater);
        assert_eq!(compare_versions("Z", "a"), Ordering::Less);
    }

    #[test]
    fn uppercase_letters_compare_among_themselves() {
        assert_eq!(compare_versions("A", "B"), Ordering::Less);
        assert_eq!(compare_versions("B", "A"), Ordering::Greater);
        assert_eq!(compare_versions("A", "A"), Ordering::Equal);
    }

    #[test]
    fn invalid_characters_are_skipped() {
        // Characters not in [a-zA-Z0-9.-~^] are skipped before comparison
        assert_eq!(compare_versions("##1.0", "1.0"), Ordering::Equal);
        assert_eq!(compare_versions("1.0##", "1.0"), Ordering::Equal);
    }

    #[test]
    fn tilde_between_two_present_tilde_strings() {
        // When both have ~ at the same position, they are consumed and comparison continues
        assert_eq!(compare_versions("1~alpha", "1~beta"), Ordering::Less);
        assert_eq!(compare_versions("1~rc1", "1~rc2"), Ordering::Less);
        assert_eq!(compare_versions("1~rc1", "1~rc1"), Ordering::Equal);
    }

    #[test]
    fn tilde_when_only_one_side_has_it() {
        // When a has ~ but b doesn't at the same position, a < b
        assert_eq!(compare_versions("1~rc1", "1.0"), Ordering::Less);
        // When b has ~ but a doesn't, a > b
        assert_eq!(compare_versions("1.0", "1~rc1"), Ordering::Greater);
    }

    #[test]
    fn optional_both_none_equal() {
        assert_eq!(compare_versions_optional(None, None), Ordering::Equal);
    }

    #[test]
    fn optional_some_vs_none_is_less() {
        // Documented behavior: (Some, None) → Less
        assert_eq!(compare_versions_optional(Some("1.0"), None), Ordering::Less);
    }

    #[test]
    fn optional_none_vs_some_is_greater() {
        // Documented behavior: (None, Some) → Greater
        assert_eq!(
            compare_versions_optional(None, Some("1.0")),
            Ordering::Greater
        );
    }

    #[test]
    fn optional_both_some_delegates_to_compare_versions() {
        assert_eq!(
            compare_versions_optional(Some("1.0"), Some("2.0")),
            Ordering::Less
        );
        assert_eq!(
            compare_versions_optional(Some("2.0"), Some("1.0")),
            Ordering::Greater
        );
        assert_eq!(
            compare_versions_optional(Some("1.0"), Some("1.0")),
            Ordering::Equal
        );
    }
}

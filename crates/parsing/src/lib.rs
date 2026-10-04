#![no_std]
extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::cmp::Reverse;
use sha2::{Digest, Sha256};

/// Stamps the `text` value with the specified `values` map. The returned value indicates
/// whether the `text` has been changed and the value that was stamped and changed.
///
/// Stamping works like this:
/// - Start with the input text.
/// - Sort all the keys in reverse length order (longest keys first)
/// - For each key, if the key is not empty, replace $KEY in the text.
/// - Each follow-up iteration acts upon the last iterations result.
/// - We keep track if the text changes during the replacement.
/// - We return both whether the text changed during any iteration and the final result.
pub fn stamp_values(values: &BTreeMap<String, String>, text: impl AsRef<str>) -> (bool, String) {
    let mut result = text.as_ref().to_string();
    let mut did_change = false;

    // Sort the keys by length. This is to ensure that we stamp the longest keys first.
    // If we did not do this, "$abc" could be stamped by "$a" into an invalid result.
    let mut keys = values.keys().collect::<Vec<_>>();

    // Sort by key length, reversed. This results in the longest keys appearing first.
    keys.sort_by_key(|key| Reverse(key.len()));

    for key in keys {
        // Empty keys are not supported.
        if key.is_empty() {
            continue;
        }

        // We can fetch the value from the map. It is verifiable that the key exists.
        let Some(value) = values.get(key) else {
            unreachable!("keys iterated over is collected on a map that cannot be modified");
        };

        let next_result = result.replace(&format!("${key}"), value);
        if result != next_result {
            did_change = true;
        }
        result = next_result;
    }
    (did_change, result)
}

/// Builds out multiple generations of `input` based on a matrix style.
/// For example, if input is: {"x": ["a", "b"], "y": ["c", "d"]}
/// It will produce:
/// x: a, y: c
/// x: a, y: d
/// x: b, y: c
/// x: b, y: d
pub fn build_matrix(input: &BTreeMap<String, Vec<String>>) -> Vec<BTreeMap<String, String>> {
    // Convert the input into a vector of tuples.
    let items: Vec<(String, Vec<String>)> = input.clone().into_iter().collect();

    // The result is a vector of maps.
    let mut result: Vec<BTreeMap<String, String>> = alloc::vec![BTreeMap::new()];

    for (key, values) in items {
        let mut new_result = Vec::new();

        // Produce all the combinations of the input values.
        for combination in &result {
            for value in &values {
                let mut new_combination = combination.clone();
                new_combination.insert(key.clone(), value.clone());
                new_result.push(new_combination);
            }
        }

        result = new_result;
    }

    result.into_iter().filter(|item| !item.is_empty()).collect()
}

/// Combine a sequence of strings into a single string, separated by spaces, ignoring empty strings.
pub fn combine_options<T: AsRef<str>>(options: impl Iterator<Item = T>) -> String {
    options
        .flat_map(|item| empty_is_none(Some(item)))
        .map(|item| item.as_ref().to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

/// The alignment of each initrd when initrds are concatenated.
const INITRD_ALIGNMENT: usize = 4;

/// Append the `initrd` to the concatenated `initrds`, padding with zeros so that it starts
/// on a 4-byte boundary. Linux only finds a cpio archive at a 4-byte aligned offset when
/// unpacking concatenated initrds, as GRUB and systemd-boot pad them the same way.
pub fn append_initrd(initrds: &mut Vec<u8>, initrd: &[u8]) {
    let padded = initrds.len().next_multiple_of(INITRD_ALIGNMENT);
    initrds.resize(padded, 0);
    initrds.extend_from_slice(initrd);
}

/// Produce a unique hash for the input.
/// This uses SHA-256, which is unique enough but relatively short.
pub fn unique_hash(input: &str) -> String {
    hex::encode(Sha256::digest(input.as_bytes()))
}

/// Filter a string-like Option `input` such that an empty string is [None].
pub fn empty_is_none<T: AsRef<str>>(input: Option<T>) -> Option<T> {
    input.filter(|input| !input.as_ref().is_empty())
}

/// Build a Xen EFI stub configuration file from pre-stamped `xen_options` and `kernel_options`.
/// The returned string is in the Xen ini-like config file format.
pub fn build_xen_config(xen_options: &str, kernel_options: &str) -> String {
    [
        // global section
        "[global]",
        // default configuration section
        "default=sprout",
        // configuration section for sprout
        "[sprout]",
        // xen options
        &format!("options={}", xen_options),
        // kernel options, stub replaces the kernel path
        // the kernel is provided via media loader
        &format!("kernel=stub {}", kernel_options),
        // required or else the last line will be ignored
        "",
    ]
    .join("\n")
}

/// Filename prefixes used to identify Linux kernel images.
/// These must be lowercase, as file names are lowercased before matching.
pub const LINUX_KERNEL_PREFIXES: &[&str] = &["vmlinuz", "image"];

/// Filename prefixes used to identify initramfs images paired with a kernel.
pub const LINUX_INITRAMFS_PREFIXES: &[&str] = &["initramfs", "initrd", "initrd.img"];

/// Check whether `name` (already lowercased) matches one of the `kernel_prefixes`,
/// either exactly or as a dash-separated prefix (e.g. `"vmlinuz-6.1"`).
/// Returns the matched prefix string if found.
pub fn match_kernel_prefix<'a>(name: &str, kernel_prefixes: &[&'a str]) -> Option<&'a str> {
    kernel_prefixes
        .iter()
        .find(|prefix| name == **prefix || name.starts_with(&format!("{}-", prefix)))
        .copied()
}

/// Generate initramfs candidate filenames by combining each entry of `initramfs_prefixes`
/// with `suffix`, and then with `suffix` followed by `.img`, which many distributions use.
/// For example, Arch Linux pairs `vmlinuz-linux` with `initramfs-linux.img`.
/// The caller is expected to check which candidates actually exist.
pub fn initramfs_candidates(
    suffix: &str,
    initramfs_prefixes: &[&str],
) -> impl Iterator<Item = String> {
    let mut candidates: Vec<String> = Vec::new();
    for extension in ["", ".img"] {
        for prefix in initramfs_prefixes {
            let candidate = format!("{}{}{}", prefix, suffix, extension);
            // Skip duplicates, like "initrd" with ".img" when "initrd.img" is also a prefix.
            if !candidates.contains(&candidate) {
                candidates.push(candidate);
            }
        }
    }
    candidates.into_iter()
}

/// Matches `text` against a glob `pattern` where `*` matches any sequence of characters,
/// including an empty one. Every other character matches itself exactly.
/// A pattern without any `*` only matches `text` when they are equal.
pub fn glob_match(pattern: &str, text: &str) -> bool {
    let pattern = pattern.as_bytes();
    let text = text.as_bytes();

    // The current position in the pattern and the text.
    let (mut p, mut t) = (0, 0);
    // The position of the last `*` in the pattern and the text position it was tried at.
    let mut backtrack: Option<(usize, usize)> = None;

    while t < text.len() {
        if p < pattern.len() && pattern[p] == b'*' {
            // Try matching the `*` against nothing first, remembering where to resume.
            backtrack = Some((p, t));
            p += 1;
        } else if p < pattern.len() && pattern[p] == text[t] {
            p += 1;
            t += 1;
        } else if let Some((star, star_text)) = backtrack {
            // Let the last `*` consume one more character and try again.
            p = star + 1;
            t = star_text + 1;
            backtrack = Some((star, star_text + 1));
        } else {
            return false;
        }
    }

    // Any remaining pattern must be made of `*` only.
    pattern[p..].iter().all(|item| *item == b'*')
}

/// One piece of a pattern for [fnmatch_ignore_case].
enum FnmatchToken {
    /// A byte that matches itself, already lowercased.
    Literal(u8),
    /// `?`, which matches any one byte.
    Any,
    /// `*`, which matches any sequence of bytes.
    Star,
    /// `[...]`, which matches a byte in any of the inclusive ranges, already lowercased.
    Class(Vec<(u8, u8)>),
    /// A pattern that can't match anything, such as one with an unterminated `[`.
    Never,
}

/// Splits `pattern` into tokens. An unterminated `[` never matches, as in systemd-boot.
fn fnmatch_tokens(pattern: &[u8]) -> Vec<FnmatchToken> {
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < pattern.len() {
        let byte = pattern[index];
        index += 1;
        match byte {
            b'*' => tokens.push(FnmatchToken::Star),
            b'?' => tokens.push(FnmatchToken::Any),
            b'\\' if index < pattern.len() => {
                tokens.push(FnmatchToken::Literal(pattern[index].to_ascii_lowercase()));
                index += 1;
            }
            b'[' => match pattern[index..].iter().position(|item| *item == b']') {
                Some(length) if length > 0 => {
                    let class = &pattern[index..index + length];
                    let mut ranges = Vec::new();
                    let mut position = 0;
                    while position < class.len() {
                        let low = class[position].to_ascii_lowercase();
                        if position + 2 < class.len() && class[position + 1] == b'-' {
                            ranges.push((low, class[position + 2].to_ascii_lowercase()));
                            position += 3;
                        } else {
                            ranges.push((low, low));
                            position += 1;
                        }
                    }
                    tokens.push(FnmatchToken::Class(ranges));
                    index += length + 1;
                }
                _ => tokens.push(FnmatchToken::Never),
            },
            other => tokens.push(FnmatchToken::Literal(other.to_ascii_lowercase())),
        }
    }
    tokens
}

/// Matches `text` against `pattern` the way systemd-boot matches entry ids: ignoring ASCII case,
/// with `*` for any sequence, `?` for any one character, `[a-z]` sets and ranges, and `\` to
/// escape the next character. Unlike [glob_match], this is for names that come from outside of
/// the Sprout configuration, such as EFI variables and loader.conf.
pub fn fnmatch_ignore_case(pattern: &str, text: &str) -> bool {
    let tokens = fnmatch_tokens(pattern.as_bytes());
    let text = text.as_bytes();

    let (mut t, mut p) = (0, 0);
    let mut backtrack: Option<(usize, usize)> = None;
    while t < text.len() {
        let byte = text[t].to_ascii_lowercase();
        let matched = match tokens.get(p) {
            Some(FnmatchToken::Literal(literal)) => *literal == byte,
            Some(FnmatchToken::Any) => true,
            Some(FnmatchToken::Class(ranges)) => ranges
                .iter()
                .any(|(low, high)| (*low..=*high).contains(&byte)),
            Some(FnmatchToken::Star) => {
                // Try matching the `*` against nothing first, remembering where to resume.
                backtrack = Some((p, t));
                p += 1;
                continue;
            }
            Some(FnmatchToken::Never) | None => false,
        };
        if matched {
            p += 1;
            t += 1;
        } else if let Some((star, star_text)) = backtrack {
            // Let the last `*` consume one more character and try again.
            p = star + 1;
            t = star_text + 1;
            backtrack = Some((star, star_text + 1));
        } else {
            return false;
        }
    }

    // Any remaining pattern must be made of `*` only.
    tokens[p..]
        .iter()
        .all(|token| matches!(token, FnmatchToken::Star))
}

/// Determines whether every key in `rule` is present in `values` with a value
/// that matches the glob pattern from `rule`. An empty rule never matches,
/// so that an empty rule cannot accidentally match everything.
pub fn rule_matches(rule: &BTreeMap<String, String>, values: &BTreeMap<String, String>) -> bool {
    !rule.is_empty()
        && rule.iter().all(|(key, pattern)| {
            values
                .get(key)
                .is_some_and(|value| glob_match(pattern, value))
        })
}

/// A single combination of variant choices produced by [build_variants].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariantCombination {
    /// The names of the chosen choices, in axis order.
    pub names: Vec<String>,
    /// The values to insert into the context for this combination.
    /// Each axis name is set to the name of its chosen choice, and then
    /// the values of each chosen choice are inserted.
    pub values: BTreeMap<String, String>,
}

/// A variant axis, which is a name and a list of choices.
/// Each choice is a name and the values it inserts.
pub type VariantAxis = (String, Vec<(String, BTreeMap<String, String>)>);

/// Builds every combination of the variant `axes`.
/// Each axis is a name and a list of choices, where each choice is a name and a set of values.
/// Combinations are produced in axis order, with the last axis varying fastest, and the
/// choices of each axis in the order they are declared.
/// No axes produce a single empty combination. An axis without any choices produces nothing.
pub fn build_variants(axes: &[VariantAxis]) -> Vec<VariantCombination> {
    let mut result = alloc::vec![VariantCombination {
        names: Vec::new(),
        values: BTreeMap::new(),
    }];

    for (axis, choices) in axes {
        let mut next = Vec::new();
        for combination in &result {
            for (name, values) in choices {
                let mut combination = combination.clone();
                combination.names.push(name.clone());
                combination.values.insert(axis.clone(), name.clone());
                combination
                    .values
                    .extend(values.iter().map(|(k, v)| (k.clone(), v.clone())));
                next.push(combination);
            }
        }
        result = next;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn stamp_replaces_known_key() {
        let values = map(&[("name", "world")]);
        let (changed, result) = stamp_values(&values, "hello $name");
        assert!(changed);
        assert_eq!(result, "hello world");
    }

    #[test]
    fn stamp_no_match_returns_unchanged() {
        let values = map(&[("name", "world")]);
        let (changed, result) = stamp_values(&values, "hello there");
        assert!(!changed);
        assert_eq!(result, "hello there");
    }

    #[test]
    fn stamp_longer_key_takes_precedence_over_shorter() {
        // Without longest-first ordering, "$ab" would be partially matched by "$a"
        let values = map(&[("a", "WRONG"), ("ab", "RIGHT")]);
        let (changed, result) = stamp_values(&values, "$ab");
        assert!(changed);
        assert_eq!(result, "RIGHT");
    }

    #[test]
    fn stamp_empty_key_is_skipped() {
        let values = map(&[("", "should-not-appear"), ("x", "val")]);
        let (_, result) = stamp_values(&values, "$x");
        assert_eq!(result, "val");
        assert!(!result.contains("should-not-appear"));
    }

    #[test]
    fn stamp_multiple_keys_replaced() {
        let values = map(&[("a", "foo"), ("b", "bar")]);
        let (changed, result) = stamp_values(&values, "$a and $b");
        assert!(changed);
        assert_eq!(result, "foo and bar");
    }

    #[test]
    fn stamp_empty_text_returns_empty() {
        let values = map(&[("a", "foo")]);
        let (changed, result) = stamp_values(&values, "");
        assert!(!changed);
        assert_eq!(result, "");
    }

    #[test]
    fn stamp_empty_map_returns_unchanged() {
        let values = map(&[]);
        let (changed, result) = stamp_values(&values, "hello $name");
        assert!(!changed);
        assert_eq!(result, "hello $name");
    }

    fn matrix_map(pairs: &[(&str, &[&str])]) -> BTreeMap<String, Vec<String>> {
        pairs
            .iter()
            .map(|(k, vs)| (k.to_string(), vs.iter().map(|v| v.to_string()).collect()))
            .collect()
    }

    #[test]
    fn matrix_single_key_produces_one_entry_per_value() {
        let input = matrix_map(&[("x", &["a", "b", "c"])]);
        let result = build_matrix(&input);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn matrix_two_keys_produces_cartesian_product() {
        let input = matrix_map(&[("x", &["a", "b"]), ("y", &["c", "d"])]);
        let result = build_matrix(&input);
        assert_eq!(result.len(), 4);
        // Every combination of x and y should be present.
        for x in &["a", "b"] {
            for y in &["c", "d"] {
                assert!(
                    result
                        .iter()
                        .any(|m| m.get("x").map(|s| s.as_str()) == Some(x)
                            && m.get("y").map(|s| s.as_str()) == Some(y))
                );
            }
        }
    }

    #[test]
    fn matrix_empty_input_produces_no_entries() {
        let input = matrix_map(&[]);
        let result = build_matrix(&input);
        assert!(result.is_empty());
    }

    #[test]
    fn matrix_key_with_empty_values_produces_no_entries() {
        let input = matrix_map(&[("x", &[])]);
        let result = build_matrix(&input);
        assert!(result.is_empty());
    }

    #[test]
    fn combine_options_joins_with_space() {
        let result = combine_options(["a", "b", "c"].iter().copied());
        assert_eq!(result, "a b c");
    }

    #[test]
    fn combine_options_skips_empty_strings() {
        let result = combine_options(["a", "", "b"].iter().copied());
        assert_eq!(result, "a b");
    }

    #[test]
    fn combine_options_all_empty_returns_empty() {
        let result = combine_options(["", ""].iter().copied());
        assert_eq!(result, "");
    }

    #[test]
    fn combine_options_empty_iterator_returns_empty() {
        let result = combine_options(core::iter::empty::<&str>());
        assert_eq!(result, "");
    }

    #[test]
    fn empty_is_none_returns_none_for_empty_string() {
        assert!(empty_is_none(Some("")).is_none());
    }

    #[test]
    fn empty_is_none_returns_some_for_nonempty_string() {
        assert_eq!(empty_is_none(Some("x")), Some("x"));
    }

    #[test]
    fn empty_is_none_passthrough_on_none() {
        assert!(empty_is_none(None::<&str>).is_none());
    }

    #[test]
    fn unique_hash_is_deterministic() {
        assert_eq!(unique_hash("hello"), unique_hash("hello"));
    }

    #[test]
    fn unique_hash_differs_for_different_inputs() {
        assert_ne!(unique_hash("hello"), unique_hash("world"));
    }

    #[test]
    fn unique_hash_empty_string() {
        // SHA-256 of "" is well-known; just check it produces a non-empty hex string.
        let h = unique_hash("");
        assert!(!h.is_empty());
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn xen_config_contains_global_and_sprout_sections() {
        let config = build_xen_config("", "");
        assert!(config.contains("[global]"));
        assert!(config.contains("[sprout]"));
        assert!(config.contains("default=sprout"));
    }

    #[test]
    fn xen_config_embeds_xen_options() {
        let config = build_xen_config("--no-real-mode --iommu=no", "");
        assert!(config.contains("options=--no-real-mode --iommu=no"));
    }

    #[test]
    fn xen_config_embeds_kernel_options() {
        let config = build_xen_config("", "quiet splash");
        assert!(config.contains("kernel=stub quiet splash"));
    }

    #[test]
    fn xen_config_ends_with_newline() {
        // Required or the last line will be ignored by the Xen config parser.
        let config = build_xen_config("", "");
        assert!(config.ends_with('\n'));
    }

    #[test]
    fn append_initrd_pads_to_four_bytes() {
        let mut initrds = Vec::new();
        append_initrd(&mut initrds, b"abcde");
        assert_eq!(initrds, b"abcde");
        append_initrd(&mut initrds, b"fg");
        assert_eq!(initrds, b"abcde\0\0\0fg");
        append_initrd(&mut initrds, b"h");
        assert_eq!(initrds, b"abcde\0\0\0fg\0\0h");
    }

    #[test]
    fn append_initrd_aligned_needs_no_padding() {
        let mut initrds = b"abcd".to_vec();
        append_initrd(&mut initrds, b"efgh");
        assert_eq!(initrds, b"abcdefgh");
    }

    #[test]
    fn kernel_prefix_exact_match() {
        assert_eq!(
            match_kernel_prefix("vmlinuz", LINUX_KERNEL_PREFIXES),
            Some("vmlinuz")
        );
    }

    #[test]
    fn kernel_prefix_dash_suffix_match() {
        assert_eq!(
            match_kernel_prefix("vmlinuz-6.1.0", LINUX_KERNEL_PREFIXES),
            Some("vmlinuz")
        );
    }

    #[test]
    fn kernel_prefix_matches_lowercased_arm64_image() {
        // Callers lowercase file names before matching, so "Image-6.1" arrives as "image-6.1".
        assert_eq!(
            match_kernel_prefix("image-6.1", LINUX_KERNEL_PREFIXES),
            Some("image")
        );
        assert_eq!(
            match_kernel_prefix("image", LINUX_KERNEL_PREFIXES),
            Some("image")
        );
    }

    #[test]
    fn kernel_prefix_case_sensitive_no_match() {
        // match_kernel_prefix expects the caller to lowercase first; uppercase input won't match.
        assert!(match_kernel_prefix("VMLINUZ-6.1", LINUX_KERNEL_PREFIXES).is_none());
    }

    #[test]
    fn kernel_prefix_no_match() {
        assert!(match_kernel_prefix("initramfs-6.1", LINUX_KERNEL_PREFIXES).is_none());
    }

    #[test]
    fn kernel_prefix_partial_no_match() {
        // "vmlinuz6.1" has no dash separator — should not match.
        assert!(match_kernel_prefix("vmlinuz6.1", LINUX_KERNEL_PREFIXES).is_none());
    }

    #[test]
    fn initramfs_candidates_with_suffix() {
        let candidates: Vec<_> = initramfs_candidates("-6.1.0", LINUX_INITRAMFS_PREFIXES).collect();
        assert_eq!(
            candidates,
            &[
                "initramfs-6.1.0",
                "initrd-6.1.0",
                "initrd.img-6.1.0",
                "initramfs-6.1.0.img",
                "initrd-6.1.0.img",
                "initrd.img-6.1.0.img",
            ]
        );
    }

    #[test]
    fn initramfs_candidates_include_img_extension() {
        // Arch Linux, and Fedora or Gentoo with dracut, add .img to the initramfs name.
        let arch: Vec<_> = initramfs_candidates("-linux", LINUX_INITRAMFS_PREFIXES).collect();
        assert!(arch.iter().any(|c| c == "initramfs-linux.img"));
        let fedora: Vec<_> =
            initramfs_candidates("-6.8.5-301.fc40.x86_64", LINUX_INITRAMFS_PREFIXES).collect();
        assert!(
            fedora
                .iter()
                .any(|c| c == "initramfs-6.8.5-301.fc40.x86_64.img")
        );
    }

    #[test]
    fn initramfs_candidates_empty_suffix() {
        let candidates: Vec<_> = initramfs_candidates("", LINUX_INITRAMFS_PREFIXES).collect();
        assert_eq!(
            candidates,
            &[
                "initramfs",
                "initrd",
                "initrd.img",
                "initramfs.img",
                "initrd.img.img"
            ]
        );
    }

    #[test]
    fn initramfs_candidates_empty_prefixes() {
        let candidates: Vec<_> = initramfs_candidates("-6.1.0", &[]).collect();
        assert!(candidates.is_empty());
    }

    #[test]
    fn fnmatch_ignores_ascii_case() {
        assert!(fnmatch_ignore_case("Foo.CONF", "foo.conf"));
        assert!(fnmatch_ignore_case("fedora-*", "Fedora-6.5.conf"));
        assert!(!fnmatch_ignore_case("fedora", "fedora.conf"));
    }

    #[test]
    fn fnmatch_question_mark_matches_one_character() {
        assert!(fnmatch_ignore_case("a?c", "abc"));
        assert!(!fnmatch_ignore_case("a?c", "ac"));
        assert!(!fnmatch_ignore_case("a?c", "abbc"));
    }

    #[test]
    fn fnmatch_brackets_match_sets_and_ranges() {
        assert!(fnmatch_ignore_case("linux-[0-9].conf", "linux-7.conf"));
        assert!(!fnmatch_ignore_case("linux-[0-9].conf", "linux-x.conf"));
        assert!(fnmatch_ignore_case("[ab]c", "bc"));
        assert!(!fnmatch_ignore_case("[ab]c", "cc"));
        assert!(fnmatch_ignore_case("[A-C]x", "bx"));
    }

    #[test]
    fn fnmatch_backslash_escapes_the_next_character() {
        assert!(fnmatch_ignore_case("a\\*b", "a*b"));
        assert!(!fnmatch_ignore_case("a\\*b", "aXb"));
        assert!(fnmatch_ignore_case("a\\?", "a?"));
    }

    #[test]
    fn fnmatch_star_backtracks() {
        assert!(fnmatch_ignore_case("*-*-1.conf", "a-b-c-1.conf"));
        assert!(fnmatch_ignore_case("*", ""));
        assert!(!fnmatch_ignore_case("*x", "abc"));
    }

    #[test]
    fn fnmatch_unterminated_bracket_matches_nothing() {
        assert!(!fnmatch_ignore_case("a[b", "a[b"));
        assert!(!fnmatch_ignore_case("a[b", "ab"));
        assert!(!fnmatch_ignore_case("a[", "a["));
    }

    #[test]
    fn fnmatch_empty_pattern_only_matches_empty_text() {
        assert!(fnmatch_ignore_case("", ""));
        assert!(!fnmatch_ignore_case("", "a"));
    }

    #[test]
    fn glob_without_star_is_exact() {
        assert!(glob_match("abc", "abc"));
        assert!(!glob_match("abc", "abcd"));
        assert!(!glob_match("abc", "ab"));
        assert!(glob_match("", ""));
        assert!(!glob_match("", "a"));
    }

    #[test]
    fn glob_trailing_star_is_prefix() {
        assert!(glob_match("fedora-*", "fedora-6.1"));
        assert!(glob_match("fedora-*", "fedora-"));
        assert!(!glob_match("fedora-*", "linux-fedora-6.1"));
    }

    #[test]
    fn glob_star_anywhere() {
        assert!(glob_match(
            "linux-*-graphics-debug",
            "linux-abc-6.1-graphics-debug"
        ));
        assert!(!glob_match(
            "linux-*-graphics-debug",
            "linux-abc-6.1-serial-debug"
        ));
        assert!(glob_match("*rescue*", "0-rescue-abc"));
        assert!(glob_match("*", ""));
        assert!(glob_match("a**b", "ab"));
        assert!(glob_match("*a*a*", "banana"));
        assert!(!glob_match("*a*a*a*a*", "banana"));
    }

    #[test]
    fn rule_requires_all_keys() {
        let values = map(&[("console", "graphics"), ("mode", "debug")]);
        assert!(rule_matches(
            &map(&[("console", "graphics"), ("mode", "deb*")]),
            &values
        ));
        assert!(!rule_matches(
            &map(&[("console", "graphics"), ("mode", "normal")]),
            &values
        ));
        assert!(!rule_matches(&map(&[("missing", "*")]), &values));
        assert!(!rule_matches(&BTreeMap::new(), &values));
    }

    fn axis(name: &str, choices: &[(&str, &[(&str, &str)])]) -> VariantAxis {
        (
            name.to_string(),
            choices
                .iter()
                .map(|(choice, values)| (choice.to_string(), map(values)))
                .collect(),
        )
    }

    #[test]
    fn variants_without_axes_produce_one_empty_combination() {
        let result = build_variants(&[]);
        assert_eq!(result.len(), 1);
        assert!(result[0].names.is_empty());
        assert!(result[0].values.is_empty());
    }

    #[test]
    fn variants_with_empty_axis_produce_nothing() {
        let result = build_variants(&[axis("a", &[("x", &[])]), axis("b", &[])]);
        assert!(result.is_empty());
    }

    #[test]
    fn variants_are_ordered_and_set_values() {
        let result = build_variants(&[
            axis(
                "console",
                &[
                    ("serial", &[("console-title", "Serial")]),
                    ("graphics", &[("console-title", "Graphics")]),
                ],
            ),
            axis(
                "mode",
                &[("normal", &[]), ("debug", &[("mode-title", " Debug")])],
            ),
        ]);
        let names: Vec<String> = result.iter().map(|c| c.names.join("-")).collect();
        assert_eq!(
            names,
            [
                "serial-normal",
                "serial-debug",
                "graphics-normal",
                "graphics-debug"
            ]
        );
        assert_eq!(
            result[3].values,
            map(&[
                ("console", "graphics"),
                ("console-title", "Graphics"),
                ("mode", "debug"),
                ("mode-title", " Debug")
            ])
        );
    }

    #[test]
    fn variant_choice_values_override_axis_name() {
        let result = build_variants(&[axis("console", &[("serial", &[("console", "custom")])])]);
        assert_eq!(
            result[0].values.get("console").map(String::as_str),
            Some("custom")
        );
    }
}

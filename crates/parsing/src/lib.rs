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
        .map(|item| item.as_ref().to_string())
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Produce a unique hash for the input.
/// This uses SHA-256, which is unique enough but relatively short.
pub fn unique_hash(input: &str) -> String {
    hex::encode(Sha256::digest(input.as_bytes()))
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
pub const LINUX_KERNEL_PREFIXES: &[&str] = &["vmlinuz", "Image"];

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
/// with `suffix`. The caller is expected to check which candidates actually exist.
pub fn initramfs_candidates<'a>(
    suffix: &'a str,
    initramfs_prefixes: &'a [&'a str],
) -> impl Iterator<Item = String> + 'a {
    initramfs_prefixes
        .iter()
        .map(move |prefix| format!("{}{}", prefix, suffix))
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
            &["initramfs-6.1.0", "initrd-6.1.0", "initrd.img-6.1.0"]
        );
    }

    #[test]
    fn initramfs_candidates_empty_suffix() {
        let candidates: Vec<_> = initramfs_candidates("", LINUX_INITRAMFS_PREFIXES).collect();
        assert_eq!(candidates, &["initramfs", "initrd", "initrd.img"]);
    }

    #[test]
    fn initramfs_candidates_empty_prefixes() {
        let candidates: Vec<_> = initramfs_candidates("-6.1.0", &[]).collect();
        assert!(candidates.is_empty());
    }
}

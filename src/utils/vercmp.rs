use std::cmp::Ordering;
use std::iter::Peekable;

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
        // If the second value is None, return that it is less than the first.
        (Some(_a), None) => Ordering::Less,
        // If the first value is None, return that it is greater than the second.
        (None, Some(_b)) => Ordering::Greater,
        // If both values are None, return that they are equal.
        (None, None) => Ordering::Equal,
    }
}

/// Compares two strings using the BLS version comparison specification.
/// See: https://uapi-group.org/specifications/specs/version_format_specification/
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

/// Compares alphabetical prefixes
/// Capital letters compare lower than lowercase letters (B < a)
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

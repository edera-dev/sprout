use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// The menu timeout of a `loader.conf` file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoaderTimeout {
    /// Show the menu for this many seconds. Zero hides the menu.
    Seconds(u64),
    /// Hide the menu unless a key is held.
    Hidden,
    /// Never show the menu.
    Disabled,
    /// Always show the menu and wait for the user.
    Force,
}

impl LoaderTimeout {
    /// Parses a `timeout` value, which is a number of seconds or one of the menu words.
    fn parse(value: &str) -> Option<Self> {
        match value {
            "menu-hidden" => Some(Self::Hidden),
            "menu-disabled" => Some(Self::Disabled),
            "menu-force" => Some(Self::Force),
            _ if !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()) => {
                value.parse().ok().map(Self::Seconds)
            }
            _ => None,
        }
    }
}

/// The settings Sprout understands from a `loader.conf` file.
/// Reference: <https://www.freedesktop.org/software/systemd/man/latest/loader.conf.html>
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LoaderConf {
    /// A glob pattern that selects the default entry, or `@saved`.
    pub default: Option<String>,
    /// A glob pattern that selects the preferred entry, or `@saved`. Unlike `default`, an entry
    /// with no boot counter tries left is not used.
    pub preferred: Option<String>,
    /// How long to show the boot menu.
    pub timeout: Option<LoaderTimeout>,
    /// Problems found while parsing, such as keys that are not supported.
    pub warnings: Vec<String>,
}

/// Removes one pair of matching single or double quotes around `value`.
fn unquote(value: &str) -> &str {
    for quote in ['"', '\''] {
        if let Some(inner) = value
            .strip_prefix(quote)
            .and_then(|rest| rest.strip_suffix(quote))
        {
            return inner;
        }
    }
    value
}

impl LoaderConf {
    /// Parses the `input` as a `loader.conf` file. Parsing never fails, as a bad setting
    /// should not prevent booting. Anything that is ignored is described in `warnings`.
    /// When a key appears more than once, the last value is used.
    pub fn parse(input: &str) -> Self {
        let mut conf = Self::default();

        // Some editors write a UTF-8 byte order mark at the start of the file.
        let input = input.strip_prefix('\u{feff}').unwrap_or(input);

        for line in input.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let (key, value) = match line.split_once(char::is_whitespace) {
                Some((key, value)) => (key, unquote(value.trim())),
                None => (line, ""),
            };

            match key {
                "default" if !value.is_empty() => conf.default = Some(value.to_string()),
                "preferred" if !value.is_empty() => conf.preferred = Some(value.to_string()),
                "timeout" => match LoaderTimeout::parse(value) {
                    Some(timeout) => conf.timeout = Some(timeout),
                    None => conf
                        .warnings
                        .push(format!("ignoring invalid loader.conf timeout '{}'", value)),
                },
                "default" | "preferred" => conf
                    .warnings
                    .push(format!("ignoring loader.conf {} without a value", key)),
                _ => conf
                    .warnings
                    .push(format!("ignoring unsupported loader.conf key '{}'", key)),
            }
        }

        conf
    }
}

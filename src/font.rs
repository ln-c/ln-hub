//! The font name and size shown in the Theme view, read from the `hl.env("...", "...")` lines of
//! the Hyprland env file.

use std::fs;
use std::ops::RangeInclusive;
use std::path::Path;

const DEFAULT_NAME: &str = "Terminess Nerd Font Propo";
const DEFAULT_SIZE: &str = "16";
/// Font sizes accepted from the size prompt.
const FONT_SIZE_RANGE: RangeInclusive<u32> = 6..=72;
/// Shown when a font size is not an integer in `FONT_SIZE_RANGE`.
pub(crate) const FONT_SIZE_ERROR: &str = "Font size must be 6-72";

/// The font name and size as the env file records them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Font {
    name: String,
    size: String,
}

impl Font {
    /// Reads `FONT_NAME` and `FONT_SIZE` from `env_file`. A missing file or variable yields the
    /// default for it.
    pub(crate) fn load(env_file: &Path) -> Font {
        let text = fs::read_to_string(env_file).unwrap_or_default();
        Font {
            name: env_value(&text, "FONT_NAME").unwrap_or_else(|| DEFAULT_NAME.to_owned()),
            size: env_value(&text, "FONT_SIZE").unwrap_or_else(|| DEFAULT_SIZE.to_owned()),
        }
    }

    /// The one-line summary drawn in the Theme view.
    pub(crate) fn summary(&self) -> String {
        format!("Font: {}, size {}", self.name, self.size)
    }
}

/// `text` as a font size: an integer within `FONT_SIZE_RANGE`.
pub(crate) fn parse_size(text: &str) -> Option<u32> {
    text.trim()
        .parse::<u32>()
        .ok()
        .filter(|n| FONT_SIZE_RANGE.contains(n))
}

/// The value of the first `hl.env("key", "value")` line for `key`.
fn env_value(text: &str, key: &str) -> Option<String> {
    let prefix = format!("hl.env(\"{key}\", \"");
    text.lines().find_map(|line| {
        let rest = line.trim_start().strip_prefix(prefix.as_str())?;
        rest.find('"').map(|end| rest[..end].to_owned())
    })
}

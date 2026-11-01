use std::fmt;

use ratatui::style::Color;

/// Palette keys, in the order they are checked for presence.
const NAMES: [&str; 18] = [
    "background",
    "foreground",
    "color0",
    "color1",
    "color2",
    "color3",
    "color4",
    "color5",
    "color6",
    "color7",
    "color8",
    "color9",
    "color10",
    "color11",
    "color12",
    "color13",
    "color14",
    "color15",
];

/// The colors of a wallust palette, resolved from `colors.sh`.
pub struct Palette {
    colors: [Color; NAMES.len()],
}

/// Why a `colors.sh` file could not be turned into a [`Palette`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaletteError {
    /// A required key was never assigned.
    Missing(String),
    /// A required key was assigned a value that is not `#rrggbb`.
    Invalid(String),
}

impl fmt::Display for PaletteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PaletteError::Missing(key) => write!(f, "palette is missing `{key}`"),
            PaletteError::Invalid(key) => write!(f, "palette value for `{key}` is not #rrggbb"),
        }
    }
}

impl std::error::Error for PaletteError {}

impl Palette {
    /// Parses wallust `colors.sh` text. Only the 18 palette keys are read; every other
    /// line (comments, blanks, `wallpaper=`, `cursor=`) is ignored.
    pub fn parse_colors_sh(s: &str) -> Result<Palette, PaletteError> {
        let mut slots: [Option<Color>; NAMES.len()] = [None; NAMES.len()];
        for line in s.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, raw)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let Some(index) = NAMES.iter().position(|name| *name == key) else {
                continue;
            };
            let color = parse_hex(unquote(raw.trim()))
                .ok_or_else(|| PaletteError::Invalid(key.to_owned()))?;
            slots[index] = Some(color);
        }

        let mut colors = [Color::Reset; NAMES.len()];
        for (index, name) in NAMES.iter().enumerate() {
            colors[index] =
                slots[index].ok_or_else(|| PaletteError::Missing((*name).to_owned()))?;
        }
        Ok(Palette { colors })
    }

    /// The color for `name` (`"background"`, `"foreground"`, `"color0"`..`"color15"`), or
    /// `None` for any other name.
    pub fn get(&self, name: &str) -> Option<Color> {
        NAMES
            .iter()
            .position(|candidate| *candidate == name)
            .map(|index| self.colors[index])
    }
}

/// Strips one matching pair of single or double quotes, if present.
fn unquote(raw: &str) -> &str {
    for quote in ['\'', '"'] {
        if let Some(inner) = raw
            .strip_prefix(quote)
            .and_then(|rest| rest.strip_suffix(quote))
        {
            return inner;
        }
    }
    raw
}

/// Decodes `#rrggbb` (case-insensitive hex digits).
fn parse_hex(value: &str) -> Option<Color> {
    let digits = value.strip_prefix('#')?;
    if digits.len() != 6 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |range: std::ops::Range<usize>| u8::from_str_radix(&digits[range], 16).ok();
    Some(Color::Rgb(channel(0..2)?, channel(2..4)?, channel(4..6)?))
}

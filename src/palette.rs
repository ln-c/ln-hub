use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

use ratatui::style::{Color, Modifier, Style};

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

/// Index of `background` in [`NAMES`].
const BACKGROUND: usize = 0;
/// Index of `color4` in [`NAMES`].
const COLOR4: usize = 6;

/// Highlight used until a valid palette has been read.
const FALLBACK_HIGHLIGHT: Style = Style::new().add_modifier(Modifier::REVERSED);

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

impl Palette {
    /// The highlight of every list: palette `background` text on a `color4` bar.
    fn highlight(&self) -> Style {
        Style::new()
            .fg(self.colors[BACKGROUND])
            .bg(self.colors[COLOR4])
    }
}

/// The palette read from a wallust `colors.sh`, refreshed when that file's mtime changes. An
/// unreadable or invalid file keeps the last good palette.
pub(crate) struct LivePalette {
    path: PathBuf,
    seen: Option<SystemTime>,
    palette: Option<Palette>,
}

impl LivePalette {
    /// Reads `path` once; a missing or invalid file leaves no palette.
    pub(crate) fn load(path: PathBuf) -> LivePalette {
        let mut live = LivePalette {
            path,
            seen: None,
            palette: None,
        };
        live.reload_if_changed();
        live
    }

    /// The list highlight from the last good palette, or the built-in fallback.
    pub(crate) fn highlight(&self) -> Style {
        self.palette
            .as_ref()
            .map_or(FALLBACK_HIGHLIGHT, Palette::highlight)
    }

    /// Rereads the file only if its mtime differs from the last one seen.
    pub(crate) fn reload_if_changed(&mut self) {
        let Ok(mtime) = fs::metadata(&self.path).and_then(|m| m.modified()) else {
            return;
        };
        if self.seen == Some(mtime) {
            return;
        }
        self.seen = Some(mtime);
        let parsed = fs::read_to_string(&self.path)
            .ok()
            .and_then(|text| Palette::parse_colors_sh(&text).ok());
        if parsed.is_some() {
            self.palette = parsed;
        }
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

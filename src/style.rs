//! Shared visual styles, kept in a leaf module so every pane can use them without depending on `app`.

use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

use ratatui::style::{Modifier, Style};

use crate::palette::Palette;

/// Highlight used until a valid palette has been read.
const FALLBACK: Style = Style::new().add_modifier(Modifier::REVERSED);

/// The highlight style of every list, derived from the wallust palette file and refreshed when
/// that file's mtime changes. An unreadable or invalid file keeps the last good style.
pub(crate) struct LiveHighlight {
    path: PathBuf,
    seen: Option<SystemTime>,
    style: Style,
}

impl LiveHighlight {
    /// Reads `path` once; a missing or invalid file yields the fallback style.
    pub(crate) fn load(path: PathBuf) -> LiveHighlight {
        let mut live = LiveHighlight {
            path,
            seen: None,
            style: FALLBACK,
        };
        live.reload_if_changed();
        live
    }

    pub(crate) fn style(&self) -> Style {
        self.style
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
        let palette = fs::read_to_string(&self.path)
            .ok()
            .and_then(|text| Palette::parse_colors_sh(&text).ok());
        if let Some(color4) = palette.and_then(|p| p.get("color4")) {
            self.style = Style::new().bg(color4);
        }
    }
}

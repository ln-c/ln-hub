//! The Theme items pane: the wallpaper list with its applied-wallpaper mark and cursor, and the
//! font summary read from the Hyprland env file.

use std::fs;
use std::path::{Path, PathBuf};

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph};

use crate::font::Font;
use crate::style::HIGHLIGHT;
use crate::wallpaper::{self, Kind, Wallpaper};

/// Row prefix for the applied wallpaper.
const CURRENT_MARK: &str = "● ";
/// Row prefix for every other wallpaper, keeping names aligned with the marked row.
const PLAIN_MARK: &str = "  ";

/// Whether `w` is the applied wallpaper.
fn is_current(current: Option<&Path>, w: &Wallpaper) -> bool {
    current == Some(w.path.as_path())
}

/// Reads the applied wallpaper's path from `current_file`.
fn load_current(current_file: &Path) -> Option<PathBuf> {
    wallpaper::read_current(current_file)
}

/// `bytes` in `ls -h` style: a plain count below 1 KiB, otherwise a power of 1024 with a unit
/// letter, rounded up. Values under 10 keep one decimal, so 1024 is `1.0K`.
fn human_size(bytes: u64) -> String {
    const UNITS: [char; 5] = ['K', 'M', 'G', 'T', 'P'];
    if bytes < 1024 {
        return bytes.to_string();
    }
    let b = u128::from(bytes);
    let mut unit = 0;
    let mut scale: u128 = 1024;
    while unit + 1 < UNITS.len() && b.div_ceil(scale) >= 1024 {
        scale *= 1024;
        unit += 1;
    }
    let tenths = (b * 10).div_ceil(scale);
    if tenths < 100 {
        format!("{}.{}{}", tenths / 10, tenths % 10, UNITS[unit])
    } else {
        format!("{}{}", b.div_ceil(scale), UNITS[unit])
    }
}

/// The detail lines for `w`: its name, size when the file can be read, kind, and thumbnail.
fn preview_lines(w: &Wallpaper) -> Vec<String> {
    let mut lines = vec![w.name.clone()];
    if let Ok(meta) = fs::metadata(&w.path) {
        lines.push(human_size(meta.len()));
    }
    lines.push(
        match w.kind {
            Kind::Image => "image",
            Kind::Video => "video",
        }
        .to_owned(),
    );
    if let Some(file) = w.thumb.as_ref().and_then(|t| t.file_name()) {
        lines.push(format!("thumb: {}", file.to_string_lossy()));
    }
    lines
}

pub(crate) struct ThemePane {
    wallpaper_dir: PathBuf,
    wallpapers: Vec<Wallpaper>,
    /// File that records the applied wallpaper, re-read by `reload`.
    current_file: PathBuf,
    /// Path of the applied wallpaper, marked with `●` when it is in the list.
    current: Option<PathBuf>,
    /// Index into `wallpapers` of the highlighted row; starts on the applied wallpaper.
    cursor: usize,
    /// Hyprland env file that records the font, re-read by `reload`.
    env_file: PathBuf,
    /// The font shown in the Theme view.
    font: Font,
}

impl ThemePane {
    pub(crate) fn new(wallpaper_dir: PathBuf, current_file: &Path, env_file: &Path) -> ThemePane {
        let wallpapers = wallpaper::discover(&wallpaper_dir);
        let current = load_current(current_file);
        let cursor = wallpapers
            .iter()
            .position(|w| is_current(current.as_deref(), w))
            .unwrap_or(0);
        ThemePane {
            wallpaper_dir,
            wallpapers,
            current_file: current_file.to_path_buf(),
            current,
            cursor,
            env_file: env_file.to_path_buf(),
            font: Font::load(env_file),
        }
    }

    /// Re-reads the applied wallpaper so the `●` mark moves, and the font from the env file; the
    /// cursor is left where it is.
    pub(crate) fn reload(&mut self) {
        self.current = load_current(&self.current_file);
        self.font = Font::load(&self.env_file);
    }

    /// The font name and size as the Theme view shows them.
    pub(crate) fn font_summary(&self) -> String {
        self.font.summary()
    }

    /// The wallpaper under the cursor; `None` when the list is empty.
    pub(crate) fn selected(&self) -> Option<&Wallpaper> {
        self.wallpapers.get(self.cursor)
    }

    /// Moves the cursor by `delta` rows, clamped to the first and last wallpaper.
    pub(crate) fn move_cursor(&mut self, delta: isize) {
        let Some(last) = self.wallpapers.len().checked_sub(1) else {
            return;
        };
        self.cursor = self.cursor.saturating_add_signed(delta).min(last);
    }

    /// Draws the wallpaper list and the details of the wallpaper under the cursor.
    pub(crate) fn render(&self, f: &mut Frame, items_area: Rect, preview_area: Rect) {
        self.render_items(f, items_area);
        self.render_preview(f, preview_area);
    }

    fn render_items(&self, f: &mut Frame, area: Rect) {
        let block = Block::bordered().title("Items");
        if self.wallpapers.is_empty() {
            let msg = format!("No wallpapers in {}", self.wallpaper_dir.display());
            f.render_widget(Paragraph::new(msg).block(block), area);
            return;
        }
        let rows: Vec<ListItem> = self
            .wallpapers
            .iter()
            .map(|w| {
                let mark = if is_current(self.current.as_deref(), w) {
                    CURRENT_MARK
                } else {
                    PLAIN_MARK
                };
                ListItem::new(format!("{mark}{}", w.name))
            })
            .collect();
        let list = List::new(rows).block(block).highlight_style(HIGHLIGHT);
        let mut state = ListState::default().with_selected(Some(self.cursor));
        f.render_stateful_widget(list, area, &mut state);
    }

    /// Draws the details of the wallpaper under the cursor; empty when there is none.
    fn render_preview(&self, f: &mut Frame, area: Rect) {
        let block = Block::bordered().title("Preview");
        let text = self
            .selected()
            .map(|w| preview_lines(w).join("\n"))
            .unwrap_or_default();
        f.render_widget(Paragraph::new(text).block(block), area);
    }
}

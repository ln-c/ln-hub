//! The Theme items pane: the wallpaper list with its applied-wallpaper mark and cursor.

use std::path::{Path, PathBuf};

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph};

use crate::style::HIGHLIGHT;
use crate::wallpaper::{self, Wallpaper};

/// Row prefix for the applied wallpaper.
const CURRENT_MARK: &str = "● ";
/// Row prefix for every other wallpaper, keeping names aligned with the marked row.
const PLAIN_MARK: &str = "  ";

/// Whether `w` is the applied wallpaper.
fn is_current(current: Option<&Path>, w: &Wallpaper) -> bool {
    current == Some(w.path.as_path())
}

pub(crate) struct ThemePane {
    wallpaper_dir: PathBuf,
    wallpapers: Vec<Wallpaper>,
    /// Path of the applied wallpaper, marked with `●` when it is in the list.
    current: Option<PathBuf>,
    /// Index into `wallpapers` of the highlighted row; starts on the applied wallpaper.
    cursor: usize,
}

impl ThemePane {
    pub(crate) fn new(wallpaper_dir: PathBuf, current_file: &Path) -> ThemePane {
        let wallpapers = wallpaper::discover(&wallpaper_dir);
        let current = wallpaper::read_current(current_file);
        let cursor = wallpapers
            .iter()
            .position(|w| is_current(current.as_deref(), w))
            .unwrap_or(0);
        ThemePane {
            wallpaper_dir,
            wallpapers,
            current,
            cursor,
        }
    }

    /// Draws the pane inside `block`, which the caller has already styled for focus.
    pub(crate) fn render(&self, f: &mut Frame, area: Rect, block: Block) {
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
}

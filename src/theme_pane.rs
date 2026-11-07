//! The Theme items pane: the wallpaper list with its applied-wallpaper mark and cursor, and the
//! font summary read from the Hyprland env file.

use std::fs;
use std::path::{Path, PathBuf};

use ratatui::Frame;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph};

use crate::font::{self, Font};
use crate::pane::{Action, Cursor, PaneAction, Program};
use crate::wallpaper::{self, Kind, Wallpaper};

/// Row prefix for the applied wallpaper.
const CURRENT_MARK: &str = "● ";
/// Row prefix for every other wallpaper, keeping names aligned with the marked row.
const PLAIN_MARK: &str = "  ";

/// Whether `w` is the applied wallpaper.
fn is_current(current: Option<&Path>, w: &Wallpaper) -> bool {
    current == Some(w.path.as_path())
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

/// What a prompt edits.
enum PromptKind {
    FontName,
    FontSize,
    Filter,
}

/// A text prompt open in the status row; typed characters go to it until Enter or Esc.
struct Prompt {
    kind: PromptKind,
    text: String,
}

impl Prompt {
    fn new(kind: PromptKind, text: String) -> Prompt {
        Prompt { kind, text }
    }

    fn line(&self) -> String {
        let label = match self.kind {
            PromptKind::FontName => "Font name",
            PromptKind::FontSize => "Font size",
            PromptKind::Filter => "Filter",
        };
        format!("{label}: {}", self.text)
    }

    /// The `ln-themectl` job the text asks for; `None` for an empty name.
    fn command(&self) -> Option<PaneAction> {
        match self.kind {
            PromptKind::FontName => font_job("-f", "font name", self.text.trim().to_owned()),
            PromptKind::FontSize => match font::parse_size(&self.text) {
                Some(size) => font_job("-s", "font size", size.to_string()),
                None => Some(PaneAction::Status(font::FONT_SIZE_ERROR.to_owned())),
            },
            PromptKind::Filter => None,
        }
    }
}

/// The `ln-themectl` job that sets a font `label` through `flag`; `None` for an empty value.
fn font_job(flag: &str, label: &str, value: String) -> Option<PaneAction> {
    if value.is_empty() {
        return None;
    }
    Some(PaneAction::Spawn {
        program: Program::Themectl,
        name: format!("{label} {value}"),
        args: vec![flag.to_owned(), value],
    })
}

pub(crate) struct ThemePane {
    wallpaper_dir: PathBuf,
    wallpapers: Vec<Wallpaper>,
    /// File that records the applied wallpaper, re-read by `reload`.
    current_file: PathBuf,
    /// Path of the applied wallpaper, marked with `●` when it is in the list.
    current: Option<PathBuf>,
    /// Index into the visible (filtered) rows of the highlighted row; starts on the applied
    /// wallpaper.
    cursor: Cursor,
    /// Kept filter: text the wallpaper names must contain (case-insensitively); empty shows
    /// every row. While the filter prompt is open its text is used instead.
    filter: String,
    /// The open prompt; it takes every key until closed.
    prompt: Option<Prompt>,
    /// Hyprland env file that records the font, re-read by `reload`.
    env_file: PathBuf,
    /// The font shown in the Theme view.
    font: Font,
}

impl ThemePane {
    pub(crate) fn new(wallpaper_dir: PathBuf, current_file: &Path, env_file: &Path) -> ThemePane {
        let wallpapers = wallpaper::discover(&wallpaper_dir);
        let current = wallpaper::read_current(current_file);
        let cursor = Cursor::at(
            wallpapers
                .iter()
                .position(|w| is_current(current.as_deref(), w))
                .unwrap_or(0),
        );
        ThemePane {
            wallpaper_dir,
            wallpapers,
            current_file: current_file.to_path_buf(),
            current,
            cursor,
            filter: String::new(),
            prompt: None,
            env_file: env_file.to_path_buf(),
            font: Font::load(env_file),
        }
    }

    /// Re-reads the applied wallpaper so the `●` mark moves, and the font from the env file; the
    /// cursor is left where it is.
    pub(crate) fn reload(&mut self) {
        self.current = wallpaper::read_current(&self.current_file);
        self.font = Font::load(&self.env_file);
    }

    /// The font name and size as the Theme view shows them.
    pub(crate) fn font_summary(&self) -> String {
        self.font.summary()
    }

    /// The filter in effect: the filter prompt's text while it is open, else the kept filter.
    fn filter_text(&self) -> &str {
        match &self.prompt {
            Some(Prompt {
                kind: PromptKind::Filter,
                text,
            }) => text,
            _ => &self.filter,
        }
    }

    /// The wallpapers whose name matches the filter, in list order.
    fn visible(&self) -> Vec<&Wallpaper> {
        let needle = self.filter_text().to_lowercase();
        self.wallpapers
            .iter()
            .filter(|w| w.name.to_lowercase().contains(&needle))
            .collect()
    }

    /// The wallpaper under the cursor; `None` when no row is visible.
    pub(crate) fn selected(&self) -> Option<&Wallpaper> {
        self.visible().get(self.cursor.get()).copied()
    }

    /// Focus left the pane: drops the filter.
    pub(crate) fn unfocus(&mut self) {
        self.clear_filter();
    }

    /// Drops the filter, keeping the highlighted wallpaper highlighted.
    fn clear_filter(&mut self) {
        let path = self.selected().map(|w| w.path.clone());
        self.filter.clear();
        self.cursor = Cursor::at(
            path.and_then(|p| self.wallpapers.iter().position(|w| w.path == p))
                .unwrap_or(0),
        );
    }

    /// Handles a key while the Theme items are focused and no prompt is open.
    pub(crate) fn handle(&mut self, action: Action) -> Option<PaneAction> {
        match action {
            Action::Down => self.cursor.move_by(1, self.visible().len()),
            Action::Up => self.cursor.move_by(-1, self.visible().len()),
            Action::Filter => {
                self.prompt = Some(Prompt::new(PromptKind::Filter, self.filter.clone()))
            }
            Action::Cancel => self.clear_filter(),
            Action::Apply => return self.apply_selected(),
            Action::FontName => {
                self.prompt = Some(Prompt::new(PromptKind::FontName, String::new()))
            }
            Action::FontSize => {
                self.prompt = Some(Prompt::new(PromptKind::FontSize, String::new()))
            }
            _ => {}
        }
        None
    }

    /// `ln-themectl -w <path>` for the wallpaper under the cursor.
    fn apply_selected(&self) -> Option<PaneAction> {
        let w = self.selected()?;
        Some(PaneAction::Spawn {
            program: Program::Themectl,
            args: vec!["-w".to_owned(), w.path.to_string_lossy().into_owned()],
            name: w.name.clone(),
        })
    }

    /// Whether a prompt is open and taking every key.
    pub(crate) fn prompt_open(&self) -> bool {
        self.prompt.is_some()
    }

    /// The prompt as shown in the status row; `None` when none is open.
    pub(crate) fn prompt_line(&self) -> Option<String> {
        self.prompt.as_ref().map(Prompt::line)
    }

    /// Keys while a prompt is open. Every printable character, including the quit key, is text.
    /// Enter closes the prompt and returns its job; while another job is in flight (`idle` is
    /// false) the prompt stays open with its text.
    pub(crate) fn handle_prompt_key(&mut self, code: KeyCode, idle: bool) -> Option<PaneAction> {
        let prompt = self.prompt.as_mut()?;
        let filtering = matches!(prompt.kind, PromptKind::Filter);
        match code {
            KeyCode::Char(c) => {
                prompt.text.push(c);
                if filtering {
                    self.cursor = Cursor::at(0);
                }
            }
            KeyCode::Backspace => {
                prompt.text.pop();
                if filtering {
                    self.cursor = Cursor::at(0);
                }
            }
            KeyCode::Esc => {
                if filtering {
                    self.clear_filter();
                }
                self.prompt = None;
            }
            KeyCode::Enter if filtering => self.filter = self.prompt.take()?.text,
            KeyCode::Enter if idle => return self.prompt.take()?.command(),
            _ => {}
        }
        None
    }

    /// Draws the wallpaper list and the details of the wallpaper under the cursor.
    pub(crate) fn render(
        &self,
        f: &mut Frame,
        items_area: Rect,
        preview_area: Rect,
        highlight: Style,
    ) {
        self.render_items(f, items_area, highlight);
        self.render_preview(f, preview_area);
    }

    fn render_items(&self, f: &mut Frame, area: Rect, highlight: Style) {
        let block = Block::bordered().title("Items");
        if self.wallpapers.is_empty() {
            let msg = format!("No wallpapers in {}", self.wallpaper_dir.display());
            f.render_widget(Paragraph::new(msg).block(block), area);
            return;
        }
        let rows: Vec<ListItem> = self
            .visible()
            .into_iter()
            .map(|w| {
                let mark = if is_current(self.current.as_deref(), w) {
                    CURRENT_MARK
                } else {
                    PLAIN_MARK
                };
                ListItem::new(format!("{mark}{}", w.name))
            })
            .collect();
        let list = List::new(rows).block(block).highlight_style(highlight);
        let mut state = ListState::default().with_selected(Some(self.cursor.get()));
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

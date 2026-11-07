use std::fs;
use std::path::PathBuf;

use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph};

use crate::config::{Config, Module};
use crate::system::System;
use crate::wallpaper::{self, Wallpaper};

/// Top-level application state: the module column, its selection, the Theme wallpaper list
/// with its cursor and the applied wallpaper, and the quit flag.
pub struct App {
    modules: Vec<Module>,
    quit_key: char,
    wallpaper_dir: PathBuf,
    wallpapers: Vec<Wallpaper>,
    /// Index into `wallpapers` of the applied wallpaper, marked with `●`.
    current: Option<usize>,
    /// Index into `wallpapers` of the highlighted row; starts on the applied wallpaper.
    cursor: usize,
    items_focused: bool,
    selected: usize,
    quit: bool,
}

impl App {
    pub fn new(cfg: Config, _sys: Box<dyn System>) -> App {
        let wallpapers = wallpaper::discover(&cfg.wallpaper_dir);
        let current = fs::read_to_string(&cfg.current_wallpaper_file)
            .ok()
            .and_then(|s| {
                let path = PathBuf::from(s.trim_end());
                wallpapers.iter().position(|w| w.path == path)
            });
        App {
            modules: cfg.modules,
            quit_key: cfg.keymap.quit,
            wallpaper_dir: cfg.wallpaper_dir,
            wallpapers,
            current,
            cursor: current.unwrap_or(0),
            items_focused: false,
            selected: 0,
            quit: false,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char(c) if c == self.quit_key => self.quit = true,
            KeyCode::Char('j') if self.selected + 1 < self.modules.len() => self.selected += 1,
            KeyCode::Char('l') => self.items_focused = true,
            _ => {}
        }
    }

    pub fn should_quit(&self) -> bool {
        self.quit
    }

    /// Periodic hook for background work; nothing to do in S1.
    pub fn tick(&mut self) {}

    pub fn render(&mut self, f: &mut Frame) {
        let [modules_area, items_area, preview_area] = Layout::horizontal([
            Constraint::Length(20),
            Constraint::Fill(1),
            Constraint::Fill(2),
        ])
        .areas(f.area());

        let items: Vec<ListItem> = self
            .modules
            .iter()
            .map(|m| ListItem::new(m.label()))
            .collect();
        let list = List::new(items)
            .block(Block::bordered().title("Modules"))
            .highlight_style(Style::new().add_modifier(Modifier::REVERSED));
        let mut state = ListState::default().with_selected(Some(self.selected));
        f.render_stateful_widget(list, modules_area, &mut state);

        let items_block = Block::bordered().title("Items");
        if self.items_focused && self.modules.get(self.selected) == Some(&Module::Theme) {
            if self.wallpapers.is_empty() {
                let msg = format!("No wallpapers in {}", self.wallpaper_dir.display());
                f.render_widget(Paragraph::new(msg).block(items_block), items_area);
            } else {
                let rows: Vec<ListItem> = self
                    .wallpapers
                    .iter()
                    .enumerate()
                    .map(|(i, w)| {
                        let mark = if self.current == Some(i) {
                            "● "
                        } else {
                            "  "
                        };
                        ListItem::new(format!("{mark}{}", w.name))
                    })
                    .collect();
                let list = List::new(rows)
                    .block(items_block)
                    .highlight_style(Style::new().add_modifier(Modifier::REVERSED));
                let mut state = ListState::default().with_selected(Some(self.cursor));
                f.render_stateful_widget(list, items_area, &mut state);
            }
        } else {
            f.render_widget(items_block, items_area);
        }
        f.render_widget(Block::bordered().title("Preview"), preview_area);
    }
}

use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, List, ListItem, ListState};

use crate::config::{Config, Module};
use crate::system::System;
use crate::theme_pane::ThemePane;

/// Highlight for the selected row of any list.
pub(crate) const HIGHLIGHT: Style = Style::new().add_modifier(Modifier::REVERSED);

/// Top-level application state: the module column and its selection, the Theme items pane,
/// and the quit flag.
pub struct App {
    modules: Vec<Module>,
    quit_key: char,
    theme: ThemePane,
    items_focused: bool,
    selected: usize,
    quit: bool,
}

impl App {
    pub fn new(cfg: Config, _sys: Box<dyn System>) -> App {
        App {
            modules: cfg.modules,
            quit_key: cfg.keymap.quit,
            theme: ThemePane::new(cfg.wallpaper_dir, &cfg.current_wallpaper_file),
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
            .highlight_style(HIGHLIGHT);
        let mut state = ListState::default().with_selected(Some(self.selected));
        f.render_stateful_widget(list, modules_area, &mut state);

        let items_block = Block::bordered().title("Items");
        if self.items_focused && self.modules.get(self.selected) == Some(&Module::Theme) {
            self.theme.render(f, items_area, items_block);
        } else {
            f.render_widget(items_block, items_area);
        }
        f.render_widget(Block::bordered().title("Preview"), preview_area);
    }
}

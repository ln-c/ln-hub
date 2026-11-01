use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, List, ListItem, ListState};

use crate::config::{Config, Module};
use crate::system::System;

/// Top-level application state: the module column, its selection and the quit flag.
pub struct App {
    modules: Vec<Module>,
    quit_key: char,
    selected: usize,
    quit: bool,
}

impl App {
    pub fn new(cfg: Config, _sys: Box<dyn System>) -> App {
        App {
            modules: cfg.modules,
            quit_key: cfg.keymap.quit,
            selected: 0,
            quit: false,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char(c) if c == self.quit_key => self.quit = true,
            KeyCode::Char('j') if self.selected + 1 < self.modules.len() => self.selected += 1,
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

        f.render_widget(Block::bordered().title("Items"), items_area);
        f.render_widget(Block::bordered().title("Preview"), preview_area);
    }
}

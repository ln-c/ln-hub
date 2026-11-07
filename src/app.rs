use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout};
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph};

use crate::config::{Config, Module};
use crate::style::HIGHLIGHT;
use crate::system::{JobId, JobState, System};
use crate::theme_pane::ThemePane;

/// An `ln-themectl` apply that has been spawned and not yet observed as finished.
struct Apply {
    job: JobId,
    name: String,
}

/// Top-level application state: the module column and its selection, the Theme items pane,
/// the in-flight apply, the status bar message, and the quit flag.
pub struct App {
    modules: Vec<Module>,
    quit_key: char,
    theme: ThemePane,
    sys: Box<dyn System>,
    items_focused: bool,
    selected: usize,
    apply: Option<Apply>,
    status: Option<String>,
    quit: bool,
}

impl App {
    pub fn new(cfg: Config, sys: Box<dyn System>) -> App {
        App {
            modules: cfg.modules,
            quit_key: cfg.keymap.quit,
            theme: ThemePane::new(cfg.wallpaper_dir, &cfg.current_wallpaper_file),
            sys,
            items_focused: false,
            selected: 0,
            apply: None,
            status: None,
            quit: false,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        let in_items = self.items_focused && self.theme_focused();
        match key.code {
            KeyCode::Char(c) if c == self.quit_key => self.quit = true,
            KeyCode::Char('j') if in_items => self.theme.move_cursor(1),
            KeyCode::Char('k') if in_items => self.theme.move_cursor(-1),
            KeyCode::Char('j') if self.selected + 1 < self.modules.len() => self.selected += 1,
            KeyCode::Char('l') => self.items_focused = true,
            KeyCode::Enter if in_items && self.apply.is_none() => self.apply_selected(),
            _ => {}
        }
    }

    pub fn should_quit(&self) -> bool {
        self.quit
    }

    /// Polls the in-flight apply; once it has finished, reports the outcome and releases the job.
    pub fn tick(&mut self) {
        let Some(apply) = self.apply.take() else {
            return;
        };
        match self.sys.poll(apply.job) {
            JobState::Running => self.apply = Some(apply),
            JobState::Done => {
                self.theme.reload_current();
                self.status = Some(format!("Applied {}", apply.name));
            }
            JobState::Failed(reason) => self.status = Some(format!("Apply failed: {reason}")),
        }
    }

    pub fn render(&mut self, f: &mut Frame) {
        let [main_area, status_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(f.area());
        let [modules_area, items_area, preview_area] = Layout::horizontal([
            Constraint::Length(20),
            Constraint::Fill(1),
            Constraint::Fill(2),
        ])
        .areas(main_area);

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
        if self.items_focused && self.theme_focused() {
            self.theme.render(f, items_area, items_block);
        } else {
            f.render_widget(items_block, items_area);
        }
        f.render_widget(Block::bordered().title("Preview"), preview_area);

        if let Some(status) = &self.status {
            f.render_widget(Paragraph::new(status.as_str()), status_area);
        }
    }

    /// Whether the module column is on the Theme module.
    fn theme_focused(&self) -> bool {
        self.modules.get(self.selected) == Some(&Module::Theme)
    }

    /// Spawns `ln-themectl -w <path>` for the wallpaper under the cursor.
    fn apply_selected(&mut self) {
        let Some(w) = self.theme.selected() else {
            return;
        };
        let name = w.name.clone();
        let path = w.path.to_string_lossy().into_owned();
        match self.sys.spawn("ln-themectl", &["-w", path.as_str()]) {
            Ok(job) => {
                self.status = Some(format!("Applying {name}…"));
                self.apply = Some(Apply { job, name });
            }
            Err(e) => self.status = Some(format!("ln-themectl not found: {e}")),
        }
    }
}

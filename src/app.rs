use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph};

use crate::config::{Config, Module};
use crate::font;
use crate::style::HIGHLIGHT;
use crate::system::{JobId, JobState, System};
use crate::theme_pane::ThemePane;

/// The program that applies wallpapers and fonts.
const THEMECTL: &str = "ln-themectl";

/// An `ln-themectl` job that has been spawned and not yet observed as finished.
struct Apply {
    job: JobId,
    /// What the job changes, as shown in the status bar: `img2.png`, `font name X`.
    name: String,
}

/// A text prompt open in the status row; typed characters go to it until Enter or Esc.
enum Prompt {
    FontName(String),
    FontSize(String),
}

impl Prompt {
    fn text_mut(&mut self) -> &mut String {
        match self {
            Prompt::FontName(text) | Prompt::FontSize(text) => text,
        }
    }

    fn line(&self) -> String {
        match self {
            Prompt::FontName(text) => format!("Font name: {text}"),
            Prompt::FontSize(text) => format!("Font size: {text}"),
        }
    }
}

/// Top-level application state: the module column and its selection, the Theme items pane,
/// the in-flight apply, the open prompt, the status bar message, and the quit flag.
pub struct App {
    modules: Vec<Module>,
    quit_key: char,
    theme: ThemePane,
    sys: Box<dyn System>,
    items_focused: bool,
    selected: usize,
    apply: Option<Apply>,
    prompt: Option<Prompt>,
    status: Option<String>,
    quit: bool,
}

impl App {
    pub fn new(cfg: Config, sys: Box<dyn System>) -> App {
        App {
            modules: cfg.modules,
            quit_key: cfg.keymap.quit,
            theme: ThemePane::new(
                cfg.wallpaper_dir,
                &cfg.current_wallpaper_file,
                &cfg.hypr_env_file,
            ),
            sys,
            items_focused: false,
            selected: 0,
            apply: None,
            prompt: None,
            status: None,
            quit: false,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if self.prompt.is_some() {
            self.handle_prompt_key(key.code);
            return;
        }
        let in_items = self.items_focused && self.theme_focused();
        match key.code {
            KeyCode::Char(c) if c == self.quit_key => self.quit = true,
            KeyCode::Char('j') if in_items => self.theme.move_cursor(1),
            KeyCode::Char('k') if in_items => self.theme.move_cursor(-1),
            KeyCode::Char('j') if self.selected + 1 < self.modules.len() => self.selected += 1,
            KeyCode::Char('l') => self.items_focused = true,
            KeyCode::Char('f') if in_items => self.prompt = Some(Prompt::FontName(String::new())),
            KeyCode::Char('s') if in_items => self.prompt = Some(Prompt::FontSize(String::new())),
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
                self.theme.reload();
                self.status = Some(format!("Applied {}", apply.name));
            }
            JobState::Failed(reason) => self.status = Some(format!("Apply failed: {reason}")),
        }
    }

    pub fn render(&mut self, f: &mut Frame) {
        let theme_active = self.items_focused && self.theme_focused();
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
        let preview_block = Block::bordered().title("Preview");
        if theme_active {
            self.theme.render(f, items_area, items_block);
            self.theme.render_preview(f, preview_area, preview_block);
        } else {
            f.render_widget(items_block, items_area);
            f.render_widget(preview_block, preview_area);
        }

        self.render_status(f, status_area, theme_active);
    }

    /// The status row: the open prompt or the status message on the left, and the font summary
    /// on the right while the Theme items are focused.
    fn render_status(&self, f: &mut Frame, area: Rect, theme_active: bool) {
        let font_text = if theme_active {
            self.theme.font_summary()
        } else {
            String::new()
        };
        let font_width = u16::try_from(Line::from(font_text.as_str()).width()).unwrap_or(u16::MAX);
        let [left_area, font_area] =
            Layout::horizontal([Constraint::Fill(1), Constraint::Length(font_width)]).areas(area);
        let left = match &self.prompt {
            Some(prompt) => prompt.line(),
            None => self.status.clone().unwrap_or_default(),
        };
        f.render_widget(Paragraph::new(left), left_area);
        f.render_widget(Paragraph::new(font_text), font_area);
    }

    /// Whether the module column is on the Theme module.
    fn theme_focused(&self) -> bool {
        self.modules.get(self.selected) == Some(&Module::Theme)
    }

    /// Keys while a prompt is open. Every printable character, including the quit key, is text.
    fn handle_prompt_key(&mut self, code: KeyCode) {
        let Some(prompt) = self.prompt.as_mut() else {
            return;
        };
        match code {
            KeyCode::Char(c) => prompt.text_mut().push(c),
            KeyCode::Backspace => {
                prompt.text_mut().pop();
            }
            KeyCode::Esc => self.prompt = None,
            KeyCode::Enter => self.submit_prompt(),
            _ => {}
        }
    }

    /// Enter in a prompt: closes it and spawns the font change. While another job is in flight,
    /// the prompt stays open with its text.
    fn submit_prompt(&mut self) {
        if self.apply.is_some() {
            return;
        }
        let Some(prompt) = self.prompt.take() else {
            return;
        };
        match prompt {
            Prompt::FontName(text) => {
                let name = text.trim();
                if !name.is_empty() {
                    self.spawn_themectl(&["-f", name], format!("font name {name}"));
                }
            }
            Prompt::FontSize(text) => match font::parse_size(&text) {
                Some(size) => {
                    let size = size.to_string();
                    self.spawn_themectl(&["-s", size.as_str()], format!("font size {size}"));
                }
                None => self.status = Some(font::FONT_SIZE_ERROR.to_owned()),
            },
        }
    }

    /// Spawns `ln-themectl` with `args` as the in-flight job, labelled `name` in the status bar.
    fn spawn_themectl(&mut self, args: &[&str], name: String) {
        match self.sys.spawn(THEMECTL, args) {
            Ok(job) => {
                self.status = Some(format!("Applying {name}…"));
                self.apply = Some(Apply { job, name });
            }
            Err(e) => self.status = Some(format!("{THEMECTL} not found: {e}")),
        }
    }

    /// Spawns `ln-themectl -w <path>` for the wallpaper under the cursor.
    fn apply_selected(&mut self) {
        let Some(w) = self.theme.selected() else {
            return;
        };
        let name = w.name.clone();
        let path = w.path.to_string_lossy().into_owned();
        self.spawn_themectl(&["-w", path.as_str()], name);
    }
}

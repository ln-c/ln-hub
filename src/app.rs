use std::io::ErrorKind;

use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph};

use crate::config::{Config, Module};
use crate::font;
use crate::monitor::{MonitorsPane, Toggle};
use crate::palette::LivePalette;
use crate::system::{JobId, JobState, System};
use crate::theme_pane::ThemePane;

/// The program that applies wallpapers and fonts.
const THEMECTL: &str = "ln-themectl";

/// The program that enables and disables monitors.
const MONITORCTL: &str = "ln-monitorctl";

/// What to refresh once a job has finished.
enum OnDone {
    ReloadTheme,
    ReloadMonitors,
}

/// A job that has been spawned and not yet observed as finished.
struct Job {
    id: JobId,
    /// What the job changes, as shown in the status bar: `img2.png`, `font name X`, `DP-1 off`.
    name: String,
    on_done: OnDone,
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
/// the in-flight job, the open prompt, the status bar message, and the quit flag.
pub struct App {
    modules: Vec<Module>,
    quit_key: char,
    theme: ThemePane,
    monitors: MonitorsPane,
    sys: Box<dyn System>,
    palette: LivePalette,
    items_focused: bool,
    selected: usize,
    job: Option<Job>,
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
            monitors: MonitorsPane::new(),
            sys,
            palette: LivePalette::load(cfg.colors_file),
            items_focused: false,
            selected: 0,
            job: None,
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
        let focused = self.focused_module();
        let in_theme = focused == Some(Module::Theme);
        let in_monitors = focused == Some(Module::Monitors);
        let idle = self.job.is_none();
        match key.code {
            KeyCode::Char(c) if c == self.quit_key => self.quit = true,
            KeyCode::Char('j') if in_theme => self.theme.move_cursor(1),
            KeyCode::Char('k') if in_theme => self.theme.move_cursor(-1),
            KeyCode::Char('j') if in_monitors => self.monitors.move_cursor(1),
            KeyCode::Char('k') if in_monitors => {
                self.monitors.move_cursor(-1);
            }
            KeyCode::Char('j') if self.selected + 1 < self.modules.len() => self.selected += 1,
            KeyCode::Char('l') => self.focus_items(),
            KeyCode::Char('f') if in_theme => self.prompt = Some(Prompt::FontName(String::new())),
            KeyCode::Char('s') if in_theme => self.prompt = Some(Prompt::FontSize(String::new())),
            KeyCode::Enter if in_theme && idle => self.apply_selected(),
            KeyCode::Enter if in_monitors && idle => {
                self.toggle_selected_monitor();
            }
            _ => {}
        }
    }

    pub fn should_quit(&self) -> bool {
        self.quit
    }

    /// Polls the in-flight job; once it has finished, reports the outcome and releases the job.
    pub fn tick(&mut self) {
        self.palette.reload_if_changed();
        let Some(job) = self.job.take() else {
            return;
        };
        match self.sys.poll(job.id) {
            JobState::Running => self.job = Some(job),
            JobState::Done => {
                match job.on_done {
                    OnDone::ReloadTheme => self.theme.reload(),
                    OnDone::ReloadMonitors => self.monitors.load(self.sys.as_ref()),
                }
                self.status = Some(format!("Applied {}", job.name));
            }
            JobState::Failed(reason) => self.status = Some(format!("Apply failed: {reason}")),
        }
    }

    pub fn render(&mut self, f: &mut Frame) {
        let highlight = self.palette.highlight();
        let theme_active = self.focused_module() == Some(Module::Theme);
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
            .highlight_style(highlight);
        let mut state = ListState::default().with_selected(Some(self.selected));
        f.render_stateful_widget(list, modules_area, &mut state);

        match self.focused_module() {
            Some(Module::Theme) => self.theme.render(f, items_area, preview_area, highlight),
            Some(Module::Monitors) => self.monitors.render(f, items_area, preview_area, highlight),
            None => {
                f.render_widget(Block::bordered().title("Items"), items_area);
                f.render_widget(Block::bordered().title("Preview"), preview_area);
            }
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

    /// The module whose panes are drawn: the selected one once the items column is focused.
    fn focused_module(&self) -> Option<Module> {
        self.modules
            .get(self.selected)
            .copied()
            .filter(|_| self.items_focused)
    }

    /// `l`: moves focus to the items column, refreshing the monitor list when entering Monitors.
    fn focus_items(&mut self) {
        self.items_focused = true;
        if self.focused_module() == Some(Module::Monitors) {
            self.monitors.load(self.sys.as_ref());
        }
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
        if self.job.is_some() {
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

    /// Spawns `prog` with `args` as the in-flight job, labelled `name` in the status bar.
    fn spawn_job(&mut self, prog: &str, args: &[&str], name: String, on_done: OnDone) {
        match self.sys.spawn(prog, args) {
            Ok(id) => {
                self.status = Some(format!("Applying {name}…"));
                self.job = Some(Job { id, name, on_done });
            }
            Err(e) if e.kind() == ErrorKind::NotFound => {
                self.status = Some(format!("{prog} not found: {e}"));
            }
            Err(e) => self.status = Some(format!("Failed to start {prog}: {e}")),
        }
    }

    /// Spawns `ln-themectl` with `args` as the in-flight job, labelled `name` in the status bar.
    fn spawn_themectl(&mut self, args: &[&str], name: String) {
        self.spawn_job(THEMECTL, args, name, OnDone::ReloadTheme);
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

    /// Toggles the monitor under the cursor through `ln-monitorctl`, unless it is the last
    /// enabled one.
    fn toggle_selected_monitor(&mut self) {
        match self.monitors.toggle() {
            Some(Toggle::Run { name, action }) => self.spawn_job(
                MONITORCTL,
                &[name.as_str(), action],
                format!("{name} {action}"),
                OnDone::ReloadMonitors,
            ),
            Some(Toggle::Refused) => {
                self.status = Some("Refusing to disable the last enabled monitor".to_owned());
            }
            None => {}
        }
    }
}

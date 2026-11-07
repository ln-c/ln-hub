use std::io::ErrorKind;

use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph};

use crate::config::{Config, Module};
use crate::monitor::MonitorsPane;
use crate::palette::LivePalette;
use crate::pane::{Action, PaneAction, Program, TOP_PREFIX};
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

/// Top-level application state: the module column and its selection, the Theme items pane,
/// the in-flight job, the status bar message, and the quit flag.
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
    status: Option<String>,
    /// A `g` was pressed; the next key completes or cancels `gg`.
    top_pending: bool,
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
            status: None,
            top_pending: false,
            quit: false,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if self.theme.prompt_open() {
            let idle = self.job.is_none();
            let pane_action = self.theme.handle_prompt_key(key.code, idle);
            self.run(pane_action);
            return;
        }
        if std::mem::take(&mut self.top_pending) {
            if key.code == KeyCode::Char(TOP_PREFIX) {
                self.handle_action(Action::Top);
            }
            return;
        }
        if key.code == KeyCode::Char(TOP_PREFIX) {
            self.top_pending = true;
            return;
        }
        let Some(action) = Action::from_key(key.code, self.quit_key) else {
            return;
        };
        self.handle_action(action);
    }

    fn handle_action(&mut self, action: Action) {
        let pane_action = match (self.focused_module(), action) {
            (_, Action::Quit) => {
                self.quit = true;
                None
            }
            (_, Action::Help) => {
                self.status = Some(format!(
                    "h/j/k/l move  gg/G top/bottom  ⏎ apply  / filter  f font  s size  ? help  {} quit",
                    self.quit_key
                ));
                None
            }
            (None, Action::Top) => {
                self.selected = 0;
                None
            }
            (None, Action::Bottom) => {
                self.selected = self.modules.len().saturating_sub(1);
                None
            }
            (_, Action::Right) => {
                self.focus_items();
                None
            }
            (Some(module), Action::Left) => {
                match module {
                    Module::Theme => self.theme.unfocus(),
                    Module::Monitors => self.monitors.unfocus(),
                }
                self.items_focused = false;
                None
            }
            (None, Action::Down) => {
                self.selected = (self.selected + 1).min(self.modules.len().saturating_sub(1));
                None
            }
            (None, Action::Up) => {
                self.selected = self.selected.saturating_sub(1);
                None
            }
            (Some(_), Action::Apply) if self.job.is_some() => None,
            (Some(Module::Theme), action) => self.theme.handle(action),
            (Some(Module::Monitors), action) => self.monitors.handle(action),
            (None, _) => None,
        };
        self.run(pane_action);
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
        let left = self
            .theme
            .prompt_line()
            .or_else(|| self.status.clone())
            .unwrap_or_default();
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

    /// Carries out what the focused pane asked for.
    fn run(&mut self, action: Option<PaneAction>) {
        match action {
            Some(PaneAction::Spawn {
                program,
                args,
                name,
            }) => {
                let (prog, on_done) = match program {
                    Program::Themectl => (THEMECTL, OnDone::ReloadTheme),
                    Program::Monitorctl => (MONITORCTL, OnDone::ReloadMonitors),
                };
                let args: Vec<&str> = args.iter().map(String::as_str).collect();
                self.spawn_job(prog, &args, name, on_done);
            }
            Some(PaneAction::Status(message)) => self.status = Some(message),
            None => {}
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
}

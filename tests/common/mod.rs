#![allow(dead_code)]
//! Shared helpers for ln-hub integration tests.
//! Only process execution (the `System` trait) is faked; the filesystem is real.

use std::io;
use std::path::PathBuf;
use std::process::Output;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use ln_hub::{App, Config, JobId, JobState, Module, System};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::{Color, Modifier};

pub type CallLog = Arc<Mutex<Vec<(String, Vec<String>)>>>;

/// Records every `run`/`spawn` call; `run` returns empty-success output by default.
#[derive(Clone)]
pub struct FakeSystem {
    pub runs: CallLog,
    pub spawns: CallLog,
    next_job: Arc<AtomicU64>,
    /// Canned stdout for `run`, keyed by `prog` (exact match). Default: empty.
    pub stdout: Arc<Mutex<Vec<(String, String)>>>,
    /// State returned by `poll` for every job. Default: Done.
    pub job_state: Arc<Mutex<JobState>>,
    /// Programs whose `run` records the call then fails with (kind, message), keyed by `prog`
    /// (exact match). Default: empty.
    pub run_errors: Arc<Mutex<Vec<(String, io::ErrorKind, String)>>>,
    /// When set, `spawn` records the call then fails with this (kind, message). Default: None.
    pub spawn_error: Arc<Mutex<Option<(io::ErrorKind, String)>>>,
}

impl FakeSystem {
    pub fn new() -> Self {
        FakeSystem {
            runs: Arc::default(),
            spawns: Arc::default(),
            next_job: Arc::new(AtomicU64::new(1)),
            stdout: Arc::default(),
            run_errors: Arc::default(),
            job_state: Arc::new(Mutex::new(JobState::Done)),
            spawn_error: Arc::default(),
        }
    }

    /// Makes every later `spawn` fail with `io::Error::new(kind, msg)`.
    pub fn set_spawn_error(&self, kind: io::ErrorKind, msg: &str) {
        *self.spawn_error.lock().unwrap() = Some((kind, msg.to_string()));
    }

    /// Makes every later `run` of `prog` fail with `io::Error::new(kind, msg)`.
    pub fn set_run_error(&self, prog: &str, kind: io::ErrorKind, msg: &str) {
        self.run_errors
            .lock()
            .unwrap()
            .push((prog.to_string(), kind, msg.to_string()));
    }

    /// Sets the canned stdout of `prog`, replacing any earlier value for it.
    pub fn set_stdout(&self, prog: &str, out: &str) {
        let mut canned = self.stdout.lock().unwrap();
        canned.retain(|(p, _)| p != prog);
        canned.push((prog.to_string(), out.to_string()));
    }

    pub fn set_job_state(&self, s: JobState) {
        *self.job_state.lock().unwrap() = s;
    }

    pub fn run_calls(&self) -> Vec<(String, Vec<String>)> {
        self.runs.lock().unwrap().clone()
    }

    pub fn spawn_calls(&self) -> Vec<(String, Vec<String>)> {
        self.spawns.lock().unwrap().clone()
    }
}

fn record(log: &CallLog, prog: &str, args: &[&str]) {
    log.lock().unwrap().push((
        prog.to_string(),
        args.iter().map(|s| s.to_string()).collect(),
    ));
}

fn success_output(stdout: Vec<u8>) -> Output {
    use std::os::unix::process::ExitStatusExt;
    Output {
        status: std::process::ExitStatus::from_raw(0),
        stdout,
        stderr: Vec::new(),
    }
}

impl System for FakeSystem {
    fn run(&self, prog: &str, args: &[&str]) -> io::Result<Output> {
        record(&self.runs, prog, args);
        if let Some((_, kind, msg)) = self
            .run_errors
            .lock()
            .unwrap()
            .iter()
            .find(|(p, _, _)| p == prog)
        {
            return Err(io::Error::new(*kind, msg.clone()));
        }
        let out = self
            .stdout
            .lock()
            .unwrap()
            .iter()
            .find(|(p, _)| p == prog)
            .map(|(_, o)| o.clone().into_bytes())
            .unwrap_or_default();
        Ok(success_output(out))
    }

    fn spawn(&self, prog: &str, args: &[&str]) -> io::Result<JobId> {
        record(&self.spawns, prog, args);
        if let Some((kind, msg)) = self.spawn_error.lock().unwrap().clone() {
            return Err(io::Error::new(kind, msg));
        }
        Ok(JobId(self.next_job.fetch_add(1, Ordering::SeqCst)))
    }

    fn poll(&self, _job: JobId) -> JobState {
        self.job_state.lock().unwrap().clone()
    }
}

pub fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// Each buffer row as a String (one cell symbol per cell, concatenated).
pub fn buffer_lines(buf: &Buffer) -> Vec<String> {
    let w = buf.area.width;
    (0..buf.area.height)
        .map(|y| {
            (0..w)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .collect()
}

/// All (x, y) cell positions where `text` starts, matching cell-by-cell
/// (so x is a true column even when borders use multi-byte glyphs).
pub fn find_text(buf: &Buffer, text: &str) -> Vec<(u16, u16)> {
    let chars: Vec<String> = text.chars().map(|c| c.to_string()).collect();
    let mut hits = Vec::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            if x as usize + chars.len() > buf.area.width as usize {
                break;
            }
            if chars
                .iter()
                .enumerate()
                .all(|(i, c)| buf[(x + i as u16, y)].symbol() == c)
            {
                hits.push((x, y));
            }
        }
    }
    hits
}

/// A fresh, unique, empty temp directory (std only; not auto-deleted).
pub fn tempdir(name: &str) -> PathBuf {
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "ln-hub-test-{}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst),
        name
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Rendered style of one cell: (fg, bg, modifier).
pub type Style = (Color, Color, Modifier);

pub fn press(app: &mut App, c: char) {
    app.handle_key(key(KeyCode::Char(c)));
}

pub fn enter(app: &mut App) {
    app.handle_key(key(KeyCode::Enter));
}

/// The whole buffer as one string, for failure messages.
pub fn dump(buf: &Buffer) -> String {
    buffer_lines(buf).join("\n")
}

/// Hits of `text` whose first cell lies in the items column x-range `[x_start, x_end)`.
pub fn items_hits(buf: &Buffer, text: &str, x_start: u16, x_end: u16) -> Vec<(u16, u16)> {
    find_text(buf, text)
        .into_iter()
        .filter(|&(x, _)| (x_start..x_end).contains(&x))
        .collect()
}

/// First hit of `text` lying ENTIRELY inside the items column x-range `[x_start, x_end)`.
pub fn items_hit_within(buf: &Buffer, text: &str, x_start: u16, x_end: u16) -> Option<(u16, u16)> {
    let len = text.chars().count() as u16;
    find_text(buf, text)
        .into_iter()
        .find(|&(x, _)| x >= x_start && x + len <= x_end)
}

/// First cell of `name` starting inside the items column `[x_start, x_end)`; panics with a
/// buffer dump when absent.
pub fn name_cell(buf: &Buffer, name: &str, x_start: u16, x_end: u16) -> (u16, u16) {
    *items_hits(buf, name, x_start, x_end)
        .first()
        .unwrap_or_else(|| {
            panic!(
                "`{name}` not found in items column (x in {x_start}..{x_end}); buffer:\n{}",
                dump(buf)
            )
        })
}

/// Row `y` of `name` in the items column `[x_start, x_end)` (see `name_cell`).
pub fn row_of(buf: &Buffer, name: &str, x_start: u16, x_end: u16) -> u16 {
    name_cell(buf, name, x_start, x_end).1
}

pub fn cell_style(buf: &Buffer, (x, y): (u16, u16)) -> Style {
    let c = &buf[(x, y)];
    (c.fg, c.bg, c.modifier)
}

/// Style of the first letter of `name` (see `name_cell`).
pub fn name_style(buf: &Buffer, name: &str, x_start: u16, x_end: u16) -> Style {
    cell_style(buf, name_cell(buf, name, x_start, x_end))
}

/// Asserts `expected` is the only highlighted row among `rows`: the other rows share one style
/// and the expected row's style differs. `locate` finds the first cell of a row's text.
pub fn assert_only_highlighted(
    buf: &Buffer,
    rows: &[&str],
    expected: &str,
    locate: impl Fn(&Buffer, &str) -> Option<(u16, u16)>,
) {
    let styles: Vec<(&str, Style)> = rows
        .iter()
        .map(|r| {
            let pos = locate(buf, r)
                .unwrap_or_else(|| panic!("`{r}` not in the items column; buffer:\n{}", dump(buf)));
            (*r, cell_style(buf, pos))
        })
        .collect();
    let others: Vec<&(&str, Style)> = styles.iter().filter(|(r, _)| *r != expected).collect();
    let hl = styles.iter().find(|(r, _)| *r == expected).unwrap().1;
    assert!(
        others.iter().all(|(_, s)| *s == others[0].1),
        "non-highlighted rows must share one style, got {styles:?}; buffer:\n{}",
        dump(buf)
    );
    assert_ne!(
        hl,
        others[0].1,
        "`{expected}` must be the highlighted row (style must differ from the others), got {styles:?}; buffer:\n{}",
        dump(buf)
    );
}

/// Asserts some rendered row contains `text` (the status bar location is not fixed by the spec).
pub fn assert_status(buf: &Buffer, text: &str) {
    assert!(
        buffer_lines(buf).iter().any(|l| l.contains(text)),
        "status `{text}` not found anywhere; buffer:\n{}",
        dump(buf)
    );
}

/// Number of `hyprctl` invocations recorded by `run`.
pub fn hyprctl_runs(sys: &FakeSystem) -> usize {
    sys.run_calls()
        .iter()
        .filter(|(prog, _)| prog == "hyprctl")
        .count()
}

/// App over a real temp wallpaper dir holding img1.png / img2.png (so a leaking Theme list would
/// be visible), with the given keys pressed in order. `modules` overrides the module order.
pub fn app_with_modules(
    name: &str,
    sys: &FakeSystem,
    modules: Option<Vec<Module>>,
    keys: &[char],
) -> App {
    let home = tempdir(&format!("{name}-home"));
    let dir = tempdir(&format!("{name}-wall"));
    for n in ["img1.png", "img2.png"] {
        std::fs::write(dir.join(n), b"x").unwrap();
    }
    let mut cfg = Config::default_for(&home);
    cfg.wallpaper_dir = dir;
    if let Some(m) = modules {
        cfg.modules = m;
    }
    let mut app = App::new(cfg, Box::new(sys.clone()));
    for &c in keys {
        press(&mut app, c);
    }
    app
}

/// `app_with_modules` with the default module order.
pub fn app_after_keys(name: &str, sys: &FakeSystem, keys: &[char]) -> App {
    app_with_modules(name, sys, None, keys)
}

/// Monitors selected (`j`) and the items column focused (`l`) under the default module order.
pub fn monitors_app(name: &str, sys: &FakeSystem) -> App {
    app_after_keys(name, sys, &['j', 'l'])
}

/// Sets the modification time of `path` explicitly, so tests never depend on fs timestamp
/// granularity or on how fast consecutive writes happen.
pub fn set_mtime(path: &std::path::Path, t: std::time::SystemTime) {
    std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(t)
        .unwrap();
}

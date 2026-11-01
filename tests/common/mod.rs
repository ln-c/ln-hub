#![allow(dead_code)]
//! Shared helpers for ln-hub integration tests.
//! Only process execution (the `System` trait) is faked; the filesystem is real.

use std::io;
use std::path::PathBuf;
use std::process::Output;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use ln_hub::{JobId, JobState, System};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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
}

impl FakeSystem {
    pub fn new() -> Self {
        FakeSystem {
            runs: Arc::default(),
            spawns: Arc::default(),
            next_job: Arc::new(AtomicU64::new(1)),
            stdout: Arc::default(),
            job_state: Arc::new(Mutex::new(JobState::Done)),
        }
    }

    pub fn set_stdout(&self, prog: &str, out: &str) {
        self.stdout
            .lock()
            .unwrap()
            .push((prog.to_string(), out.to_string()));
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

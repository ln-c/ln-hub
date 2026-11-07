//! Real process execution backed by `std::process`.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::PathBuf;
use std::process::{Child, Command, Output, Stdio};

use crate::system::{JobId, JobState, System};

/// Runs real processes; spawned jobs append their stdout/stderr to a log file.
pub struct ProcSystem {
    log: PathBuf,
    next_id: Cell<u64>,
    jobs: RefCell<HashMap<u64, Child>>,
}

impl ProcSystem {
    pub fn new(log: PathBuf) -> Self {
        ProcSystem {
            log,
            next_id: Cell::new(1),
            jobs: RefCell::new(HashMap::new()),
        }
    }

    fn open_log(&self) -> io::Result<File> {
        if let Some(dir) = self.log.parent() {
            fs::create_dir_all(dir)?;
        }
        OpenOptions::new().append(true).create(true).open(&self.log)
    }

    /// Stdout and stderr sinks; both null when the log is unusable.
    fn sinks(&self) -> (Stdio, Stdio) {
        match self.open_log().and_then(|f| Ok((f.try_clone()?, f))) {
            Ok((out, err)) => (out.into(), err.into()),
            Err(_) => (Stdio::null(), Stdio::null()),
        }
    }
}

impl System for ProcSystem {
    fn run(&self, prog: &str, args: &[&str]) -> io::Result<Output> {
        Command::new(prog).args(args).output()
    }

    fn spawn(&self, prog: &str, args: &[&str]) -> io::Result<JobId> {
        let (out, err) = self.sinks();
        let child = Command::new(prog)
            .args(args)
            .stdin(Stdio::null())
            .stdout(out)
            .stderr(err)
            .spawn()?;
        let id = self.next_id.get();
        self.next_id.set(id + 1);
        self.jobs.borrow_mut().insert(id, child);
        Ok(JobId(id))
    }

    fn poll(&self, job: JobId) -> JobState {
        let mut jobs = self.jobs.borrow_mut();
        let Some(child) = jobs.get_mut(&job.0) else {
            return JobState::Failed(format!("unknown job {}", job.0));
        };
        match child.try_wait() {
            Ok(None) => JobState::Running,
            Ok(Some(status)) if status.success() => JobState::Done,
            Ok(Some(status)) => JobState::Failed(match status.code() {
                Some(code) => format!("exit {code}"),
                None => "killed by signal".to_string(),
            }),
            Err(e) => JobState::Failed(e.to_string()),
        }
    }
}

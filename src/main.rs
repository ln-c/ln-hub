use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::process::{Child, Command, Output, Stdio};

use ln_hub::{App, Config, JobId, JobState, System};
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyEventKind};

/// Real process execution backed by `std::process`.
struct ProcSystem {
    next_id: Cell<u64>,
    jobs: RefCell<HashMap<u64, Child>>,
}

impl ProcSystem {
    fn new() -> Self {
        ProcSystem {
            next_id: Cell::new(1),
            jobs: RefCell::new(HashMap::new()),
        }
    }
}

impl System for ProcSystem {
    fn run(&self, prog: &str, args: &[&str]) -> io::Result<Output> {
        Command::new(prog).args(args).output()
    }

    fn spawn(&self, prog: &str, args: &[&str]) -> io::Result<JobId> {
        let child = Command::new(prog)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
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
            Ok(Some(status)) => JobState::Failed(format!("exit {}", status.code().unwrap_or(-1))),
            Err(e) => JobState::Failed(e.to_string()),
        }
    }
}

fn run(terminal: &mut DefaultTerminal, app: &mut App) -> io::Result<()> {
    while !app.should_quit() {
        terminal.draw(|f| app.render(f))?;
        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            app.handle_key(key);
        }
    }
    Ok(())
}

fn main() -> io::Result<()> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?;
    let mut app = App::new(Config::default_for(&home), Box::new(ProcSystem::new()));

    let mut terminal = ratatui::init();
    let result = run(&mut terminal, &mut app);
    ratatui::restore();
    result
}

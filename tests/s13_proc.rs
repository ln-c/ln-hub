//! S13-bin (R2): the real `ProcSystem` (public lib type) exercised against real processes.
//! `spawn` starts a detached child whose stdout/stderr are appended to a log file; `poll`
//! reports Running / Done / Failed("exit N").

mod common;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use ln_hub::{JobId, JobState, ProcSystem, System};

/// Polls every ~10ms until the job is no longer Running; panics after 5s.
fn wait_done(sys: &ProcSystem, id: JobId) -> JobState {
    let start = Instant::now();
    loop {
        let st = sys.poll(id);
        if st != JobState::Running {
            return st;
        }
        if start.elapsed() > Duration::from_secs(5) {
            panic!("job {id:?} still Running after 5s");
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn new_sys(name: &str) -> (ProcSystem, PathBuf) {
    let log = common::tempdir(name).join("jobs.log");
    (ProcSystem::new(log.clone()), log)
}

fn read_log(log: &Path) -> String {
    fs::read_to_string(log).unwrap_or_default()
}

// Breaks if `run` does not capture stdout or does not report the exit status.
#[test]
fn run_captures_stdout_and_success_status() {
    let (sys, _) = new_sys("proc-run");
    let out = sys.run("sh", &["-c", "echo out"]).unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), "out\n");
    assert!(out.status.success());
}

// Breaks if a successful spawned job never reaches Done (e.g. stuck Running or Failed).
#[test]
fn spawn_true_reaches_done() {
    let (sys, _) = new_sys("proc-done");
    let id = sys.spawn("true", &[]).unwrap();
    assert_eq!(wait_done(&sys, id), JobState::Done);
}

// Breaks if a nonzero exit is not reported as Failed("exit <code>") with the exact message.
#[test]
fn spawn_nonzero_exit_fails_with_exit_code() {
    let (sys, _) = new_sys("proc-fail");
    let id = sys.spawn("sh", &["-c", "exit 3"]).unwrap();
    assert_eq!(wait_done(&sys, id), JobState::Failed("exit 3".into()));
}

// Breaks if a missing program is swallowed or mapped to a different io error kind.
#[test]
fn spawn_missing_program_is_not_found() {
    let (sys, _) = new_sys("proc-missing");
    let err = sys
        .spawn("ln-hub-definitely-missing-prog", &[])
        .unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::NotFound);
}

// Breaks if polling an id that was never issued panics or reports Running/Done.
#[test]
fn poll_unknown_job_is_failed() {
    let (sys, _) = new_sys("proc-unknown");
    assert!(matches!(sys.poll(JobId(999)), JobState::Failed(_)));
}

// Breaks if job stdout/stderr are discarded (Stdio::null) instead of going to the log.
#[test]
fn job_stdout_and_stderr_go_to_log() {
    let (sys, log) = new_sys("proc-log");
    let id = sys
        .spawn("sh", &["-c", "echo to-out; echo to-err >&2"])
        .unwrap();
    assert_eq!(wait_done(&sys, id), JobState::Done);
    let content = read_log(&log);
    assert!(
        content.contains("to-out"),
        "stdout missing from log: {content:?}"
    );
    assert!(
        content.contains("to-err"),
        "stderr missing from log: {content:?}"
    );
}

// Breaks if missing parent directories of the log path are not created.
#[test]
fn log_parent_dirs_are_created() {
    let root = common::tempdir("proc-parents");
    let log = root.join("a").join("b").join("jobs.log");
    let sys = ProcSystem::new(log.clone());
    let id = sys.spawn("sh", &["-c", "echo nested"]).unwrap();
    assert_eq!(wait_done(&sys, id), JobState::Done);
    assert!(read_log(&log).contains("nested"));
}

// Breaks if the log is truncated on each spawn instead of appended to.
#[test]
fn log_is_appended_not_truncated() {
    let (sys, log) = new_sys("proc-append");
    fs::write(&log, "earlier\n").unwrap();
    let a = sys.spawn("sh", &["-c", "echo later"]).unwrap();
    assert_eq!(wait_done(&sys, a), JobState::Done);
    let b = sys.spawn("sh", &["-c", "echo again"]).unwrap();
    assert_eq!(wait_done(&sys, b), JobState::Done);
    let content = read_log(&log);
    for want in ["earlier", "later", "again"] {
        assert!(
            content.contains(want),
            "{want:?} missing from log: {content:?}"
        );
    }
}

// Breaks if an unusable log path (parent is a regular file) makes spawn fail instead of
// degrading gracefully.
#[test]
fn unusable_log_path_does_not_break_jobs() {
    let root = common::tempdir("proc-badlog");
    let notadir = root.join("notadir");
    fs::write(&notadir, "x").unwrap();
    let sys = ProcSystem::new(notadir.join("jobs.log"));
    let id = sys.spawn("true", &[]).unwrap();
    assert_eq!(wait_done(&sys, id), JobState::Done);
}

// Breaks if job ids are reused (e.g. a constant or non-incrementing counter).
#[test]
fn distinct_spawns_get_distinct_ids() {
    let (sys, _) = new_sys("proc-ids");
    let a = sys.spawn("true", &[]).unwrap();
    let b = sys.spawn("true", &[]).unwrap();
    assert_ne!(a, b);
    assert_eq!(wait_done(&sys, a), JobState::Done);
    assert_eq!(wait_done(&sys, b), JobState::Done);
}

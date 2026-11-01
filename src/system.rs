use std::io;
use std::process::Output;

/// Handle to a job started with [`System::spawn`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JobId(pub u64);

/// Progress of a job started with [`System::spawn`].
#[derive(Debug, Clone, PartialEq)]
pub enum JobState {
    Running,
    Done,
    Failed(String),
}

/// External process boundary: blocking queries, async jobs and their polling.
pub trait System {
    /// Blocking query; returns the captured output.
    fn run(&self, prog: &str, args: &[&str]) -> io::Result<Output>;
    /// Starts an async job and returns its handle.
    fn spawn(&self, prog: &str, args: &[&str]) -> io::Result<JobId>;
    /// Reports the current state of a job.
    fn poll(&self, job: JobId) -> JobState;
}

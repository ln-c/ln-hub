//! S13-bin (R1): the `ln-hub [--version|--config <path>]` command line, exercised on the real
//! binary. Every case here must exit before the terminal UI starts, so no tty is needed.
//!
//! `run_cli` never lets a case hang: stdin is null, and a child still running after
//! `TIMEOUT` (e.g. a binary that ignored its arguments and entered the TUI) is killed and the
//! test fails with a clear message.

mod common;

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(5);

struct Cli {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

fn drain<R: Read + Send + 'static>(mut r: R) -> thread::JoinHandle<String> {
    thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = r.read_to_end(&mut buf);
        String::from_utf8_lossy(&buf).into_owned()
    })
}

/// Runs the binary with `args` and `HOME=home`, no XDG override, null stdin.
fn run_cli(args: &[&str], home: &Path) -> Cli {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ln-hub"))
        .args(args)
        .env("HOME", home)
        .env_remove("XDG_CONFIG_HOME")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn ln-hub");
    let out = drain(child.stdout.take().unwrap());
    let err = drain(child.stderr.take().unwrap());

    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().expect("try_wait") {
            break status;
        }
        if start.elapsed() > TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            panic!("ln-hub {args:?} timed out (binary entered the TUI?)");
        }
        thread::sleep(Duration::from_millis(20));
    };
    Cli {
        code: status.code(),
        stdout: out.join().unwrap(),
        stderr: err.join().unwrap(),
    }
}

fn write_file(dir: &Path, name: &str, body: &str) -> PathBuf {
    let p = dir.join(name);
    fs::write(&p, body).unwrap();
    p
}

#[test]
fn version_prints_name_and_version_and_exits_0() {
    // Catches: --version ignored (TUI starts) or printing the wrong text/exit code.
    let home = common::tempdir("cli-version");
    let r = run_cli(&["--version"], &home);
    assert_eq!(r.stdout, "ln-hub 0.1.0\n", "stderr: {}", r.stderr);
    assert_eq!(r.code, Some(0));
}

#[test]
fn missing_config_file_reports_config_and_path_exit_2() {
    // Catches: --config <missing> silently falling back to defaults / launching the TUI.
    let home = common::tempdir("cli-missing");
    let path = home.join("nope.toml");
    let path_s = path.to_str().unwrap();
    let r = run_cli(&["--config", path_s], &home);
    assert_eq!(r.code, Some(2), "stderr: {}", r.stderr);
    assert!(r.stderr.contains("config"), "stderr: {}", r.stderr);
    assert!(r.stderr.contains(path_s), "stderr: {}", r.stderr);
}

#[test]
fn config_flag_with_unknown_module_reports_it_exit_2() {
    // Catches: --config file not loaded/validated, so the bad module is never reported.
    let home = common::tempdir("cli-unknown-module");
    let cfg = write_file(&home, "c.toml", "modules = [\"Audio\"]\n");
    let r = run_cli(&["--config", cfg.to_str().unwrap()], &home);
    assert_eq!(r.code, Some(2), "stderr: {}", r.stderr);
    assert!(r.stderr.contains("unknown module: Audio"), "stderr: {}", r.stderr);
}

#[test]
fn default_config_path_under_home_is_read() {
    // Catches: no-arg start ignoring $HOME/.config/ln-hub/config.toml.
    let home = common::tempdir("cli-default-config");
    let dir = home.join(".config/ln-hub");
    fs::create_dir_all(&dir).unwrap();
    write_file(&dir, "config.toml", "modules = [\"Audio\"]\n");
    let r = run_cli(&[], &home);
    assert_eq!(r.code, Some(2), "stderr: {}", r.stderr);
    assert!(r.stderr.contains("unknown module: Audio"), "stderr: {}", r.stderr);
}

#[test]
fn invalid_toml_reports_invalid_config_exit_2() {
    // Catches: parse errors not surfaced (or surfaced without the `invalid config` prefix).
    let home = common::tempdir("cli-invalid-toml");
    let cfg = write_file(&home, "bad.toml", "modules = [\n");
    let r = run_cli(&["--config", cfg.to_str().unwrap()], &home);
    assert_eq!(r.code, Some(2), "stderr: {}", r.stderr);
    assert!(r.stderr.contains("invalid config"), "stderr: {}", r.stderr);
}

#[test]
fn unknown_argument_prints_usage_to_stderr_exit_2() {
    // Catches: unknown flags ignored (TUI starts) or usage written to stdout.
    let home = common::tempdir("cli-bogus");
    let r = run_cli(&["--bogus"], &home);
    assert_eq!(r.code, Some(2), "stderr: {}", r.stderr);
    assert!(r.stderr.contains("usage"), "stderr: {}", r.stderr);
    assert_eq!(r.stdout, "");
}

#[test]
fn config_flag_without_path_prints_usage_exit_2() {
    // Catches: `--config` with no value being ignored or panicking instead of a usage error.
    let home = common::tempdir("cli-config-nopath");
    let r = run_cli(&["--config"], &home);
    assert_eq!(r.code, Some(2), "stderr: {}", r.stderr);
    assert!(r.stderr.contains("usage"), "stderr: {}", r.stderr);
    assert_eq!(r.stdout, "");
}

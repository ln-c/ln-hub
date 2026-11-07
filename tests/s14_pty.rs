//! S14-picker-query: the real `ln-hub` binary in a 24x80 pty whose terminal never answers the
//! capability query. The UI must draw, stay responsive, and never eat a key.
//!
//! Safety: PATH is an empty temp dir and HOME a temp dir, so the TUI can neither reach
//! `hyprctl` / `ln-themectl` / `ln-monitorctl` nor touch the real config. Only `j` and `q` are
//! ever typed. The master is drained on a thread so the child never blocks on a write, and a
//! child still alive at the end of a case is killed (no hang).

mod common;

use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use portable_pty::{Child, CommandBuilder, PtySize, native_pty_system};

const FRAME_TIMEOUT: Duration = Duration::from_secs(5);

struct Session {
    child: Box<dyn Child + Send + Sync>,
    writer: Box<dyn Write + Send>,
    out: Arc<Mutex<Vec<u8>>>,
    start: Instant,
    // Keeps the master (and so the pty) alive for the whole session.
    _master: Box<dyn portable_pty::MasterPty + Send>,
}

impl Session {
    fn spawn(name: &str) -> Session {
        let home = common::tempdir(&format!("{name}-home"));
        let empty_path = common::tempdir(&format!("{name}-path"));
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("open pty");
        let mut cmd = CommandBuilder::new(env!("CARGO_BIN_EXE_ln-hub"));
        cmd.env("HOME", &home);
        cmd.env("PATH", &empty_path);
        cmd.env("TERM", "xterm-256color");
        cmd.env_remove("XDG_CONFIG_HOME");
        cmd.cwd(Path::new(&home));
        let start = Instant::now();
        let child = pair.slave.spawn_command(cmd).expect("spawn ln-hub in pty");
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader().expect("pty reader");
        let writer = pair.master.take_writer().expect("pty writer");
        let out = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&out);
        thread::spawn(move || {
            let mut buf = [0u8; 4096];
            while let Ok(n) = reader.read(&mut buf) {
                if n == 0 {
                    break;
                }
                sink.lock().unwrap().extend_from_slice(&buf[..n]);
            }
        });
        Session {
            child,
            writer,
            out,
            start,
            _master: pair.master,
        }
    }

    fn output(&self) -> Vec<u8> {
        self.out.lock().unwrap().clone()
    }

    /// Blocks until the first frame (the `Theme` module title) is in the output.
    fn wait_first_frame(&self) {
        let deadline = Instant::now() + FRAME_TIMEOUT;
        while Instant::now() < deadline {
            if String::from_utf8_lossy(&self.output()).contains("Theme") {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!(
            "no first frame within {FRAME_TIMEOUT:?}; output: {:?}",
            String::from_utf8_lossy(&self.output())
        );
    }

    /// Sleeps until `at` after spawn (no-op if already past).
    fn sleep_until(&self, at: Duration) {
        if let Some(rest) = at.checked_sub(self.start.elapsed()) {
            thread::sleep(rest);
        }
    }

    fn send(&mut self, bytes: &[u8]) {
        self.writer.write_all(bytes).expect("write key");
        self.writer.flush().expect("flush key");
    }

    /// Waits up to `within` for the child to exit; returns its exit code, or `None` after
    /// killing it (so a failing case never leaves a process behind).
    fn wait_exit(&mut self, within: Duration) -> Option<u32> {
        let deadline = Instant::now() + within;
        loop {
            if let Some(status) = self.child.try_wait().expect("try_wait") {
                // Let the drain thread collect the tail of the output.
                thread::sleep(Duration::from_millis(100));
                return Some(status.exit_code());
            }
            if Instant::now() >= deadline {
                let _ = self.child.kill();
                let _ = self.child.wait();
                return None;
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}

/// The last 160 bytes of the output, lossy, for failure messages (the full frame is noise).
fn tail(out: &[u8]) -> String {
    String::from_utf8_lossy(&out[out.len().saturating_sub(160)..]).into_owned()
}

fn assert_no_sixel(out: &[u8]) {
    assert!(
        !out.windows(2).any(|w| w == b"\x1bP"),
        "output has a sixel/DCS start `ESC P`; output tail: {:?}",
        tail(out)
    );
}

#[test]
fn silent_terminal_draws_first_frame_and_q_at_300ms_exits_within_1s() {
    // Catches: the query freezing the UI for ~2 s and eating the `q` typed inside that window
    // (the process then never quits on the first press), or sixel output with no reply.
    let mut s = Session::spawn("pty-quit");
    s.wait_first_frame();
    s.sleep_until(Duration::from_millis(300));
    s.send(b"q");
    let code = s.wait_exit(Duration::from_secs(1));
    assert_eq!(
        code,
        Some(0),
        "ln-hub did not exit 0 within 1 s of `q` (key eaten or UI frozen?); output tail: {:?}",
        tail(&s.output())
    );
    assert_no_sixel(&s.output());
}

#[test]
fn keys_after_the_query_deadline_are_not_lost_j_then_q_exits_on_first_q() {
    // Catches: an orphaned or concurrent stdin reader swallowing keys so `q` needs a second press.
    let mut s = Session::spawn("pty-jq");
    s.wait_first_frame();
    s.sleep_until(Duration::from_millis(400));
    s.send(b"j");
    thread::sleep(Duration::from_millis(150));
    s.send(b"q");
    let code = s.wait_exit(Duration::from_secs(1));
    assert_eq!(
        code,
        Some(0),
        "`q` after `j` did not exit on the first press (a key was eaten); output tail: {:?}",
        tail(&s.output())
    );
}

#[test]
fn first_key_long_after_the_query_window_is_not_eaten_by_a_leftover_reader() {
    // Catches: a reader thread left blocked on stdin after the query timed out, which eats the
    // first key typed later (here a lone `q` 3 s after spawn).
    let mut s = Session::spawn("pty-late-q");
    s.wait_first_frame();
    s.sleep_until(Duration::from_secs(3));
    s.send(b"q");
    let code = s.wait_exit(Duration::from_secs(1));
    assert_eq!(
        code,
        Some(0),
        "a lone `q` long after the query window did not exit (eaten by a leftover reader); output tail: {:?}",
        tail(&s.output())
    );
}

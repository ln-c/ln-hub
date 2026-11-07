//! The shared pty harness: the real `ln-hub` binary in a 24x80 pty (moved out of `s14_pty.rs` so
//! S16 and S18 share it).
//!
//! Safety: PATH is an empty temp dir and HOME a temp dir, so the TUI can neither reach
//! `hyprctl` / `ln-themectl` / `ln-monitorctl` nor touch the real config. The master is drained on
//! a thread so the child never blocks on a write, and a child still alive at the end of a case is
//! killed by `wait_exit` (no hang).

use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use portable_pty::{Child, CommandBuilder, PtySize, native_pty_system};

use super::tempdir;

pub const FRAME_TIMEOUT: Duration = Duration::from_secs(5);

pub struct Session {
    child: Box<dyn Child + Send + Sync>,
    writer: Box<dyn Write + Send>,
    out: Arc<Mutex<Vec<u8>>>,
    // Keeps the master (and so the pty) alive for the whole session.
    _master: Box<dyn portable_pty::MasterPty + Send>,
}

impl Session {
    pub fn spawn(name: &str) -> Session {
        Session::spawn_with_home(name, |_| {})
    }

    /// Like `spawn`, but `setup` runs on the fresh HOME before the binary starts (to put files in
    /// it, e.g. a wallpaper).
    pub fn spawn_with_home(name: &str, setup: impl FnOnce(&Path)) -> Session {
        let home = tempdir(&format!("{name}-home"));
        setup(&home);
        let empty_path = tempdir(&format!("{name}-path"));
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
            _master: pair.master,
        }
    }

    pub fn output(&self) -> Vec<u8> {
        self.out.lock().unwrap().clone()
    }

    /// Blocks until the first frame (the `Theme` module title) is in the output.
    pub fn wait_first_frame(&self) {
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

    /// Polls the output until it contains `needle` or `within` has passed; true if it does.
    pub fn wait_for(&self, needle: &[u8], within: Duration) -> bool {
        let deadline = Instant::now() + within;
        loop {
            if contains(&self.output(), needle) {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    /// Blocks until `Ready` (S16: drawn once the startup query thread has exited, before the
    /// event loop reads keys) is in the output, so a key sent afterwards cannot land in the query
    /// window. Panics, killing the child, if it never shows.
    pub fn wait_ready(&mut self) {
        if !self.wait_for(b"Ready", FRAME_TIMEOUT) {
            let out = self.output();
            let _ = self.wait_exit(Duration::ZERO);
            panic!(
                "no `Ready` within {FRAME_TIMEOUT:?}; output tail: {:?}",
                tail(&out)
            );
        }
    }

    pub fn send(&mut self, bytes: &[u8]) {
        self.writer.write_all(bytes).expect("write key");
        self.writer.flush().expect("flush key");
    }

    /// Waits up to `within` for the child to exit; returns its exit code, or `None` after
    /// killing it (so a failing case never leaves a process behind).
    pub fn wait_exit(&mut self, within: Duration) -> Option<u32> {
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

/// Whether `needle` occurs in `hay` (an empty needle occurs everywhere).
pub fn contains(hay: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    hay.windows(needle.len()).any(|w| w == needle)
}

/// The last 160 bytes of the output, lossy, for failure messages (the full frame is noise).
pub fn tail(out: &[u8]) -> String {
    String::from_utf8_lossy(&out[out.len().saturating_sub(160)..]).into_owned()
}

pub fn assert_no_sixel(out: &[u8]) {
    assert!(
        !out.windows(2).any(|w| w == b"\x1bP"),
        "output has a sixel/DCS start `ESC P`; output tail: {:?}",
        tail(out)
    );
}

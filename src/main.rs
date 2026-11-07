mod image_view;

use std::io::{self, BufWriter, Stdout, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use image_view::{ImageView, TimingLog, fmt_ms};
use ln_hub::{App, Config, ProcSystem};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event::{self, Event, KeyEventKind};

/// Forwards to `inner`; with `seen` set (timing on) it also raises the flag when a sixel start
/// `ESC P` passes through, also when the two bytes come in separate writes.
struct SixelSpy<W: Write> {
    inner: W,
    seen: Option<Arc<AtomicBool>>,
    last_was_esc: bool,
}

impl<W: Write> Write for SixelSpy<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.inner.write(buf)?;
        if let Some(seen) = &self.seen {
            // Only the bytes actually written count.
            let sent = &buf[..n];
            let split = self.last_was_esc && sent.first() == Some(&b'P');
            if split || sent.windows(2).any(|w| w == b"\x1bP") {
                seen.store(true, Ordering::Relaxed);
            }
            if let Some(&last) = sent.last() {
                self.last_was_esc = last == 0x1b;
            }
        }
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// The spy sits under the `BufWriter`, so it sees a few large chunks per frame.
type Term = Terminal<CrosstermBackend<BufWriter<SixelSpy<Stdout>>>>;

/// `timing` and `seen` (the flag the terminal's `SixelSpy` raises) are both `Some` or both `None`.
fn run(
    terminal: &mut Term,
    app: &mut App,
    timing: Option<TimingLog>,
    seen: Option<Arc<AtomicBool>>,
) -> io::Result<()> {
    // The first frame is drawn before any image work (picker query, worker threads) happens.
    app.init_started();
    terminal.draw(|f| app.render(f))?;
    let mut images = ImageView::new(timing.clone());
    let mut was_pending = true;
    while !app.should_quit() {
        images.poll();
        if was_pending && !images.query_pending() {
            was_pending = false;
            app.init_finished(Instant::now());
        }
        // Timing off costs this one `Option` check per frame.
        let start = seen.as_ref().map(|_| Instant::now());
        // `draw` flushes, so a slow terminal write is inside the timed call.
        terminal.draw(|f| {
            app.render(f);
            if let Some(preview) = app.preview_image() {
                images.render(f, &preview);
            }
        })?;
        if let (Some(start), Some(seen), Some(timing)) = (start, &seen, &timing)
            && seen.swap(false, Ordering::Relaxed)
        {
            timing.line(&format!("draw {}", fmt_ms(start.elapsed())));
        }
        // A finished encode is picked up by `poll` within ~10 ms while one is pending.
        let wait = if images.encode_pending() {
            Duration::from_millis(10)
        } else {
            Duration::from_millis(100)
        };
        // The query thread owns stdin until it reports done; never two concurrent readers.
        if images.query_pending() {
            thread::sleep(Duration::from_millis(50));
        } else if event::poll(wait)?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            app.handle_key(key);
        }
        app.tick();
    }
    Ok(())
}

const USAGE: &str = "usage: ln-hub [--version|--config <path>]";

enum Cli {
    Version,
    Run { config: Option<PathBuf> },
}

fn parse_args(args: impl Iterator<Item = String>) -> Option<Cli> {
    let mut args = args;
    match (args.next().as_deref(), args.next(), args.next()) {
        (None, _, _) => Some(Cli::Run { config: None }),
        (Some("--version"), None, _) => Some(Cli::Version),
        (Some("--config"), Some(path), None) => Some(Cli::Run {
            config: Some(PathBuf::from(path)),
        }),
        _ => None,
    }
}

/// Loads the config from `path`, or the default location (falling back to built-in defaults
/// when that file does not exist). Errors are returned as printable messages.
fn load_config(path: Option<PathBuf>, home: &Path) -> Result<Config, String> {
    let explicit = path.is_some();
    let path = path.unwrap_or_else(|| home.join(".config/ln-hub/config.toml"));
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound && !explicit => {
            return Ok(Config::default_for(home));
        }
        Err(e) => return Err(format!("ln-hub: config {}: {e}", path.display())),
    };
    Config::from_toml(&text, home).map_err(|e| format!("ln-hub: config {}: {e}", path.display()))
}

fn main() -> ExitCode {
    let config = match parse_args(std::env::args().skip(1)) {
        None => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
        Some(Cli::Version) => {
            println!("ln-hub {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        Some(Cli::Run { config }) => config,
    };
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        eprintln!("ln-hub: HOME is not set");
        return ExitCode::from(2);
    };
    let config = match load_config(config, &home) {
        Ok(config) => config,
        Err(msg) => {
            eprintln!("{msg}");
            return ExitCode::from(2);
        }
    };
    let mut app = App::new(
        config,
        Box::new(ProcSystem::new(home.join(".cache/ln-hub/jobs.log"))),
    );

    // Opened before the terminal is set up; an unopenable path turns timing off silently.
    let timing = TimingLog::from_env();
    let seen = timing.as_ref().map(|_| Arc::new(AtomicBool::new(false)));
    // For its panic hook, raw mode and the alternate screen; the bin draws through its own
    // terminal below so the sixel bytes can be noticed on their way out.
    let _ = ratatui::init();
    let spy = SixelSpy {
        inner: io::stdout(),
        seen: seen.clone(),
        last_was_esc: false,
    };
    // The terminal is dropped (and its buffer flushed) before `restore` leaves the alternate
    // screen.
    let result = Terminal::new(CrosstermBackend::new(BufWriter::new(spy)))
        .and_then(|mut terminal| run(&mut terminal, &mut app, timing, seen));
    ratatui::restore();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ln-hub: {e}");
            ExitCode::FAILURE
        }
    }
}

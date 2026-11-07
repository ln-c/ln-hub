//! S22-sixel-off-ui-thread: the real `ln-hub` binary in a 24x80 pty whose terminal answers the
//! startup query with sixel support (the S18 technique: wait for the DA1 request `ESC [ c`, then
//! write `ESC [ ? 62 ; 4 c` and `ESC [ 6 ; 16 ; 8 t`), run with `LN_HUB_TIMING=<file>`.
//!
//! The observable interface S22 adds is the timing log: one `encode <cols>x<rows> <ms>ms` line per
//! finished encode (written by the encode worker) and one `draw <ms>ms` line per frame that
//! emitted sixel bytes. The first test is the RED: before S22 no `encode` line is ever written.
//! The other three are guards (green from the start is fine): fast hovering still ends with a
//! sixel, `q` during a pending encode exits at once, and an unwritable timing path is harmless.
//!
//! Safety: PATH is an empty temp dir and HOME a temp dir (see `common::pty`); the timing file
//! lives under HOME. Only the reply bytes and keys are ever written.

mod common;

use std::ffi::OsString;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use common::pty::{Session, find};

const DA1_REQUEST: &[u8] = b"\x1b[c";
const REPLY: &[u8] = b"\x1b[?62;4c\x1b[6;16;8t";
const SIXEL_START: &[u8] = b"\x1bP";
/// The start of the help line (`?`). Short on purpose: ratatui diff-draws the status row, so a
/// longer needle with an unchanged cell inside would not be contiguous in the raw bytes (S18).
const HELP_LINE: &[u8] = b"h/j/k/l";
const TIMING_FILE: &str = "timing.log";

/// A real PNG at `Pictures/Wallpapers/<name>` under `home`: a gradient with low-amplitude noise
/// from a fixed xorshift seed (no `rand` dependency). Not a flat colour, so the encode does real
/// work; not full noise, which decodes slowly in a debug build.
fn write_png(home: &Path, name: &str, w: u32, h: u32) {
    let dir = home.join("Pictures/Wallpapers");
    std::fs::create_dir_all(&dir).unwrap();
    let mut state: u32 = 0x9e37_79b9;
    let img = image::RgbImage::from_fn(w, h, |x, y| {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        let noise = (state % 16) as u8;
        let r = (x * 255 / w.max(1)) as u8;
        let g = (y * 255 / h.max(1)) as u8;
        let b = ((x + y) * 255 / (w + h).max(1)) as u8;
        image::Rgb([
            r.saturating_add(noise),
            g.saturating_add(noise),
            b.saturating_sub(noise),
        ])
    });
    img.save(dir.join(name)).expect("write wallpaper png");
}

/// The large wallpaper the RED test mandates, so a debug-build encode takes a visible time.
fn write_large(home: &Path) {
    write_png(home, "a.png", 2400, 1600);
}

/// `LN_HUB_TIMING=<home>/timing.log`.
fn timing_env(home: &Path) -> Vec<(String, OsString)> {
    vec![(
        "LN_HUB_TIMING".to_string(),
        home.join(TIMING_FILE).into_os_string(),
    )]
}

/// The timing log's lines; empty when the file does not exist (yet), so a missing log reads as
/// "no line" rather than an io error.
fn timing_lines(home: &Path) -> Vec<String> {
    std::fs::read_to_string(home.join(TIMING_FILE))
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

fn is_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

/// `\d+\.\dms`: a millisecond count with exactly one decimal.
fn is_ms(s: &str) -> bool {
    let Some(num) = s.strip_suffix("ms") else {
        return false;
    };
    let Some((int, frac)) = num.split_once('.') else {
        return false;
    };
    is_digits(int) && frac.len() == 1 && is_digits(frac)
}

/// `^encode \d+x\d+ \d+\.\dms$`, by hand (no regex crate).
fn is_encode_line(line: &str) -> bool {
    let Some(rest) = line.strip_prefix("encode ") else {
        return false;
    };
    let Some((dims, ms)) = rest.split_once(' ') else {
        return false;
    };
    let Some((cols, rows)) = dims.split_once('x') else {
        return false;
    };
    is_digits(cols) && is_digits(rows) && is_ms(ms)
}

/// `^draw \d+\.\dms$`, by hand.
fn is_draw_line(line: &str) -> bool {
    line.strip_prefix("draw ").is_some_and(is_ms)
}

/// Polls the timing log until some line satisfies `pred` or `deadline` passes.
fn wait_line(home: &Path, pred: fn(&str) -> bool, deadline: Instant) -> bool {
    loop {
        if timing_lines(home).iter().any(|l| pred(l)) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(10));
    }
}

/// Answers the startup query with sixel support and waits for `Ready` (keys sent afterwards
/// cannot land in the query window).
fn answer_query(s: &mut Session) {
    if !s.wait_for(DA1_REQUEST, Duration::from_secs(5)) {
        s.fail("the binary never emitted the DA1 request `ESC [ c`");
    }
    s.send(REPLY);
    s.wait_ready();
}

fn quit_exits_0(s: &mut Session, when: &str) {
    s.send(b"q");
    let code = s.wait_exit(Duration::from_secs(1));
    if code != Some(0) {
        s.fail(&format!(
            "`q` {when} did not exit 0 within 1 s on the first press (got {code:?})"
        ));
    }
}

#[test]
fn l_encodes_off_the_ui_thread_and_logs_encode_and_draw_times() {
    // Catches: no `encode` line in the timing log (the encode is not on the worker, or the worker
    // does not log), a malformed line, no sixel for the hovered wallpaper, no `draw` line for the
    // frame that flushed the sixel, `?` not handled, or `q` needing a second press.
    //
    // The responsiveness assertion (`?` right after `l` shows the help line) is weak on purpose:
    // before S22 a `?` sent right after `l` is also handled, because the decode (already
    // off-thread) has not finished yet, so a pty test cannot pin "a key is handled while an encode
    // is pending" without a race. The real measurement is the VM check (S24).
    let mut s = Session::spawn_with_env("s22-encode", write_large, timing_env);
    answer_query(&mut s);
    let home = s.home().to_path_buf();

    s.send(b"l");
    let deadline = Instant::now() + Duration::from_secs(5);
    s.send(b"?");
    if !s.wait_for(HELP_LINE, Duration::from_secs(2)) {
        s.fail("the help line (`?` right after `l`) did not appear within 2 s");
    }

    if !wait_line(&home, is_encode_line, deadline) {
        let lines = timing_lines(&home);
        s.fail(&format!(
            "no `encode <cols>x<rows> <ms>ms` line in {TIMING_FILE} within 5 s of `l`; lines: {lines:?}"
        ));
    }
    let left = deadline.saturating_duration_since(Instant::now());
    if !s.wait_for(SIXEL_START, left) {
        s.fail("no sixel start `ESC P` within 5 s of `l` (a.png hovered)");
    }
    // The `draw` line is written right after the `terminal.draw` call that flushed the sixel
    // returns, so it can trail the bytes in the pty by a moment.
    if !wait_line(&home, is_draw_line, Instant::now() + Duration::from_secs(1)) {
        let lines = timing_lines(&home);
        s.fail(&format!(
            "no `draw <ms>ms` line in {TIMING_FILE} after the sixel frame; lines: {lines:?}"
        ));
    }

    quit_exits_0(&mut s, "after the sixel frame");
}

#[test]
fn fast_hovering_ends_with_the_hovered_wallpaper_drawn() {
    // Catches: a response routed to (or dropped for) the wrong entry when hovering moves faster
    // than the encodes, leaving the last hovered wallpaper never drawn.
    let mut s = Session::spawn_with_env(
        "s22-hover",
        |home| {
            for name in ["a.png", "b.png", "c.png"] {
                write_png(home, name, 64, 64);
            }
        },
        timing_env,
    );
    answer_query(&mut s);

    s.send(b"l");
    s.send(b"j");
    s.send(b"j");
    s.send(b"k");
    let n = s.output().len();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if find(&s.output()[n..], SIXEL_START).is_some() {
            break;
        }
        if Instant::now() >= deadline {
            s.fail("no sixel `ESC P` drawn within 5 s after the last hover key (`k`, b.png)");
        }
        thread::sleep(Duration::from_millis(10));
    }

    quit_exits_0(&mut s, "after fast hovering");
}

#[test]
fn q_during_a_pending_encode_exits_0_within_1s() {
    // Catches: the encode worker (or a blocking wait on it) holding up the exit.
    let mut s = Session::spawn_with_env("s22-quit", write_large, timing_env);
    answer_query(&mut s);

    s.send(b"l");
    quit_exits_0(&mut s, "right after `l` (decode/encode in flight)");
}

#[test]
fn an_unwritable_timing_path_does_not_stop_the_app() {
    // Catches: an `LN_HUB_TIMING` path that cannot be opened stopping the app, breaking the
    // sixel path, or creating the missing directories.
    let mut s = Session::spawn_with_env(
        "s22-unwritable",
        |home| write_png(home, "a.png", 64, 64),
        |home| {
            vec![(
                "LN_HUB_TIMING".to_string(),
                home.join("missing/dir/timing.log").into_os_string(),
            )]
        },
    );
    answer_query(&mut s);

    s.send(b"l");
    if !s.wait_for(SIXEL_START, Duration::from_secs(5)) {
        s.fail("no sixel start `ESC P` within 5 s of `l` with an unwritable timing path");
    }
    quit_exits_0(&mut s, "with an unwritable timing path");

    let missing = s.home().join("missing");
    assert!(
        !missing.exists(),
        "an unwritable timing path created {}",
        missing.display()
    );
}

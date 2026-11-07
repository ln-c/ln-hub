//! S18-review-followups (a): the real `ln-hub` binary in a 24x80 pty whose terminal ANSWERS the
//! startup capability query with sixel support.
//!
//! Characterization, not a RED: it describes behavior that already works and is green from the
//! start. It still earns its place because the early exit of the query window and the positive
//! sixel path had no pty coverage (S14 only covers a silent terminal, where no `ESC P` must
//! appear).
//!
//! The fake terminal waits for `ESC [ c` (the DA1 request, last in the query) in the output, then
//! writes `ESC [ ? 62 ; 4 c` (DA1 with sixel) and `ESC [ 6 ; 16 ; 8 t` (cell size 8x16).
//!
//! Safety: PATH is an empty temp dir and HOME a temp dir (see `common::pty`). Only the reply
//! bytes, `l` and `q` are ever written.

mod common;

use std::path::Path;
use std::thread;
use std::time::Duration;

use common::pty::{Session, contains, tail};

const DA1_REQUEST: &[u8] = b"\x1b[c";
const REPLY: &[u8] = b"\x1b[?62;4c\x1b[6;16;8t";
const SIXEL_START: &[u8] = b"\x1bP";
/// The start of the help line (`?`), which a reply byte leaking into the event loop as a key would
/// open. Deliberately a short token with no spaces: ratatui diff-draws the status row, so a cell
/// that did not change (the space in `h/j/k/l move`) is skipped with a cursor move, and a longer
/// needle would never be contiguous in the raw bytes. `h/j/k/l` is absent from every frame drawn
/// without the help line.
const HELP_LINE: &[u8] = b"h/j/k/l";

/// A real (decodable) 16x16 PNG at `Pictures/Wallpapers/a.png` under `home`.
fn write_wallpaper(home: &Path) {
    let dir = home.join("Pictures/Wallpapers");
    std::fs::create_dir_all(&dir).unwrap();
    image::RgbImage::from_pixel(16, 16, image::Rgb([200, 40, 40]))
        .save(dir.join("a.png"))
        .expect("write a.png");
}

/// Fails unless the cumulative output has no help line: a reply byte (`?`) that reached the event
/// loop as a key would have opened it.
fn assert_no_leaked_key(s: &mut Session, when: &str) {
    if contains(&s.output(), HELP_LINE) {
        s.fail(&format!(
            "the help line is in the output {when}: a reply byte reached the event loop as a key"
        ));
    }
}

#[test]
fn answered_query_exits_early_leaks_no_keys_and_l_draws_a_sixel() {
    // Catches: the query window not ending early once DA1 is seen (Ready only after the 250 ms
    // deadline, > 200 ms), the reply bytes reaching the event loop as keys (`?` opens the help
    // line, a stray `q`/`l` changes state), a missing sixel for the hovered wallpaper after `l`,
    // or `q` needing a second press.
    let mut s = Session::spawn_with_home("s18-pty-sixel", write_wallpaper);

    if !s.wait_for(DA1_REQUEST, Duration::from_secs(5)) {
        s.fail("the binary never emitted the DA1 request `ESC [ c`");
    }
    // The timer starts when the request is seen; the reply goes out right away.
    s.send(REPLY);
    if !s.wait_for(b"Ready", Duration::from_millis(200)) {
        s.fail("`Ready` not in the output within 200 ms of the DA1 reply (no early exit?)");
    }

    // The loop draws `Ready` and only then reads (and would handle) a leaked key and redraw, so
    // give it a moment before looking; the later checks cover anything slower.
    thread::sleep(Duration::from_millis(150));
    assert_no_leaked_key(&mut s, "after Ready");
    if contains(&s.output(), SIXEL_START) {
        s.fail(
            "a sixel `ESC P` is in the output before `l` (nothing is hovered yet), so the positive check below would prove nothing",
        );
    }

    s.send(b"l");
    if !s.wait_for(SIXEL_START, Duration::from_secs(2)) {
        s.fail("no sixel start `ESC P` within 2 s of `l` (a.png hovered)");
    }
    assert_no_leaked_key(&mut s, "after l");

    s.send(b"q");
    let code = s.wait_exit(Duration::from_secs(1));
    assert_no_leaked_key(&mut s, "by the end");
    assert_eq!(
        code,
        Some(0),
        "`q` did not exit 0 within 1 s on the first press (a key was eaten); output tail: {:?}",
        tail(&s.output())
    );
}

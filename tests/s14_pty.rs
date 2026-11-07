//! S14-picker-query: the real `ln-hub` binary in a 24x80 pty whose terminal never answers the
//! capability query. The UI must draw, stay responsive, and never eat a key.
//!
//! Safety: PATH is an empty temp dir and HOME a temp dir, so the TUI can neither reach
//! `hyprctl` / `ln-themectl` / `ln-monitorctl` nor touch the real config. Only `j` and `q` are
//! ever typed. The master is drained on a thread so the child never blocks on a write, and a
//! child still alive at the end of a case is killed (no hang).

mod common;

use std::thread;
use std::time::Duration;

use common::pty::{Session, assert_no_sixel, tail};

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

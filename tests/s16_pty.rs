//! S16-init-status: the real `ln-hub` binary in a 24x80 pty whose terminal never answers the
//! capability query. The first frame must carry `Initializing…`, `Ready` must follow within 1 s of
//! it, and `Ready` must not swallow a key.
//!
//! The checks look at the raw byte stream (ratatui writes a styled run of adjacent cells without
//! cursor moves, so `Initializing…` is contiguous; every cell of `Ready` differs from the cell of
//! `Initializing…` it replaces, so `Ready` is contiguous too). Only `q` is ever typed.

mod common;

use std::time::Duration;

use common::pty::{Session, assert_no_sixel, find, tail};

const INITIALIZING: &str = "Initializing…";
const READY: &str = "Ready";

#[test]
fn the_first_frame_carries_initializing() {
    // Catches: no `Initializing…` in the bin output (init_started never wired before the first
    // draw, or the text not drawn), or sixel output with no terminal reply.
    let mut s = Session::spawn("s16-pty-init");
    s.wait_first_frame();
    // Same flush as the `Theme` title; the grace only covers the drain thread's chunking.
    let seen = s.wait_for(INITIALIZING.as_bytes(), Duration::from_millis(100));
    let out = s.output();
    let _ = s.wait_exit(Duration::ZERO); // kill the child, whatever the outcome
    assert!(
        seen,
        "`{INITIALIZING}` not in the output with the first frame; output tail: {:?}",
        tail(&out)
    );
    assert_no_sixel(&out);
}

#[test]
fn ready_follows_within_1s_of_the_first_frame_after_initializing() {
    // Catches: init_finished never called by the bin loop, Ready arriving later than 1 s after
    // the first frame, or Ready drawn before Initializing….
    let mut s = Session::spawn("s16-pty-ready");
    s.wait_first_frame();
    let ready = s.wait_for(READY.as_bytes(), Duration::from_secs(1));
    let out = s.output();
    let _ = s.wait_exit(Duration::ZERO);
    assert!(
        ready,
        "`{READY}` not in the output within 1 s of the first frame; output tail: {:?}",
        tail(&out)
    );
    let init_at = find(&out, INITIALIZING.as_bytes());
    let ready_at = find(&out, READY.as_bytes());
    assert!(
        init_at.is_some() && init_at < ready_at,
        "`{INITIALIZING}` must come before `{READY}` in the byte stream (got {init_at:?} vs {ready_at:?}); output tail: {:?}",
        tail(&out)
    );
    assert_no_sixel(&out);
}

#[test]
fn q_sent_once_ready_is_in_the_output_exits_0_within_1s() {
    // Catches: Ready (or the loop that shows it) swallowing the first key: `q` typed while Ready
    // is on screen must still quit.
    let mut s = Session::spawn("s16-pty-q");
    s.wait_first_frame();
    let ready = s.wait_for(READY.as_bytes(), Duration::from_secs(2));
    if !ready {
        let _ = s.wait_exit(Duration::ZERO);
        panic!(
            "`{READY}` never appeared in the output; output tail: {:?}",
            tail(&s.output())
        );
    }
    s.send(b"q");
    let code = s.wait_exit(Duration::from_secs(1));
    assert_eq!(
        code,
        Some(0),
        "`q` after `{READY}` did not exit 0 within 1 s (key swallowed?); output tail: {:?}",
        tail(&s.output())
    );
    assert_no_sixel(&s.output());
}

//! S16-init-status: while the startup terminal query is pending the status row shows
//! `Initializing…`; when it finishes it shows `Ready`, which clears on the first key (still
//! handled) or after ~1 s, then the normal status returns. The startup text is the lowest-priority
//! text on the left of the status row: it never hides or overwrites help, a job status or a prompt.
//!
//! Time is injected (`init_finished(now)`, `tick_at(now)`): tests move it with `Instant + Duration`
//! and never sleep. Only `System` is faked; the wallpaper dir is a real temp dir.
//!
//! The status row is the LAST row of an 80x24 render; the startup text is left-aligned in it.

mod common;

use std::time::{Duration, Instant};

use common::*;
use ln_hub::{App, JobState};

const INITIALIZING: &str = "Initializing…";
const READY: &str = "Ready";
const HELP_TEXT: &str = "h/j/k/l move";
const MS500: Duration = Duration::from_millis(500);
const MS1500: Duration = Duration::from_millis(1500);

fn fresh(name: &str) -> (App, FakeSystem) {
    let sys = FakeSystem::new();
    (app_after_keys(name, &sys, &[]), sys)
}

fn assert_row_starts_with(app: &mut App, text: &str) {
    let buf = draw(app);
    let row = buffer_lines(&buf).last().unwrap().trim_end().to_string();
    assert!(
        row.starts_with(text),
        "status row must start with `{text}`, got `{row}`; buffer:\n{}",
        dump(&buf)
    );
}

fn assert_row_empty(app: &mut App) {
    let buf = draw(app);
    let row = buffer_lines(&buf).last().unwrap().trim().to_string();
    assert_eq!(row, "", "status row must be empty; buffer:\n{}", dump(&buf));
}

/// Panics with a buffer dump if `text` appears on ANY row.
fn assert_nowhere(app: &mut App, text: &str) {
    let buf = draw(app);
    assert!(
        !buffer_lines(&buf).iter().any(|l| l.contains(text)),
        "`{text}` must not be shown; buffer:\n{}",
        dump(&buf)
    );
}

fn assert_no_startup_text(app: &mut App) {
    assert_nowhere(app, INITIALIZING);
    assert_nowhere(app, READY);
}

fn assert_module_highlighted(app: &mut App, expected: &str) {
    let buf = draw(app);
    assert_only_highlighted(&buf, &["Theme", "Monitors"], expected, |b, n| {
        items_hits(b, n, 0, 20).first().copied()
    });
}

#[test]
fn a_fresh_app_shows_neither_initializing_nor_ready() {
    // Catches: the startup text shown by default (every earlier test relies on an empty status row).
    let (mut app, _sys) = fresh("s16-fresh");
    assert_no_startup_text(&mut app);
    assert_row_empty(&mut app);
}

#[test]
fn init_started_shows_initializing_left_aligned_on_the_last_row() {
    // Catches: no text after init_started, or text on another row / not left-aligned.
    let (mut app, _sys) = fresh("s16-started");
    app.init_started();
    assert_row_starts_with(&mut app, INITIALIZING);
}

#[test]
fn initializing_is_not_cleared_by_tick_at_any_later_instant() {
    // Catches: `Initializing…` expiring on a timer (only init_finished may end it).
    let (mut app, _sys) = fresh("s16-started-tick");
    app.init_started();
    let now = Instant::now();
    for later in [
        Duration::from_millis(10),
        MS1500,
        Duration::from_secs(60),
        Duration::from_secs(3600),
    ] {
        app.tick_at(now + later);
        assert_row_starts_with(&mut app, INITIALIZING);
    }
    app.tick();
    assert_row_starts_with(&mut app, INITIALIZING);
}

#[test]
fn init_finished_replaces_initializing_with_ready() {
    // Catches: Ready never shown, or Initializing… left on screen after init_finished.
    let (mut app, _sys) = fresh("s16-finished");
    app.init_started();
    app.init_finished(Instant::now());
    assert_row_starts_with(&mut app, READY);
    assert_nowhere(&mut app, INITIALIZING);
}

#[test]
fn init_finished_without_init_started_still_shows_ready() {
    // Catches: Ready only reachable through a prior init_started.
    let (mut app, _sys) = fresh("s16-finished-alone");
    app.init_finished(Instant::now());
    assert_row_starts_with(&mut app, READY);
}

#[test]
fn ready_survives_500ms_and_is_gone_after_1500ms() {
    // Catches: Ready never expiring, or expiring at once / before ~1 s; the row must be empty
    // (as with no status) afterwards.
    let (mut app, _sys) = fresh("s16-ready-expiry");
    let t0 = Instant::now();
    app.init_started();
    app.init_finished(t0);
    app.tick_at(t0 + MS500);
    assert_row_starts_with(&mut app, READY);
    app.tick_at(t0 + MS1500);
    assert_no_startup_text(&mut app);
    assert_row_empty(&mut app);
}

#[test]
fn tick_right_after_init_finished_keeps_ready() {
    // Catches: tick() expiring Ready at once (e.g. a stale or zero instant) instead of using the
    // wall clock: tick() == tick_at(Instant::now()).
    let (mut app, _sys) = fresh("s16-tick-ready");
    app.init_started();
    app.init_finished(Instant::now());
    app.tick();
    assert_row_starts_with(&mut app, READY);
}

#[test]
fn first_key_clears_ready_without_a_tick_and_question_mark_is_still_handled() {
    // Catches: Ready surviving the first key, or the clearing key being swallowed (no help line).
    let (mut app, _sys) = fresh("s16-key-help");
    app.init_started();
    app.init_finished(Instant::now());
    assert_row_starts_with(&mut app, READY);
    press(&mut app, '?');
    assert_nowhere(&mut app, READY);
    assert_row_starts_with(&mut app, HELP_TEXT);
}

#[test]
fn first_key_clears_ready_and_j_still_moves_the_module_cursor() {
    // Catches: the key that clears Ready being dropped (cursor stays on Theme), or Ready kept.
    let (mut app, _sys) = fresh("s16-key-j");
    assert_module_highlighted(&mut app, "Theme");
    app.init_started();
    app.init_finished(Instant::now());
    press(&mut app, 'j');
    assert_nowhere(&mut app, READY);
    assert_module_highlighted(&mut app, "Monitors");
}

#[test]
fn first_key_clears_ready_and_the_quit_key_still_quits() {
    // Catches: Ready swallowing the quit key (the pty `q` case at the App level).
    let (mut app, _sys) = fresh("s16-key-quit");
    app.init_started();
    app.init_finished(Instant::now());
    assert!(!app.should_quit());
    press(&mut app, 'q');
    assert!(
        app.should_quit(),
        "the quit key must be handled, not swallowed by Ready"
    );
    assert_nowhere(&mut app, READY);
}

#[test]
fn after_ready_cleared_by_key_the_normal_status_returns() {
    // Catches: a status row stuck blank/Ready after the key clear: a later `?` must show the help
    // and an Enter apply must show `Applying …`.
    let sys = FakeSystem::new();
    let mut app = app_after_keys("s16-after-key", &sys, &[]);
    app.init_started();
    app.init_finished(Instant::now());
    press(&mut app, 'l'); // first key: clears Ready, focuses the items
    assert_no_startup_text(&mut app);
    press(&mut app, '?');
    assert_row_starts_with(&mut app, HELP_TEXT);
    enter(&mut app);
    assert_row_starts_with(&mut app, "Applying img1.png…");
    assert_no_startup_text(&mut app);
}

#[test]
fn after_ready_expired_by_time_the_normal_status_returns() {
    // Catches: expiry leaving the status machinery broken (help / Applying no longer shown).
    let sys = FakeSystem::new();
    let mut app = app_after_keys("s16-after-time", &sys, &['l']);
    let t0 = Instant::now();
    app.init_started();
    app.init_finished(t0);
    app.tick_at(t0 + MS1500);
    press(&mut app, '?');
    assert_row_starts_with(&mut app, HELP_TEXT);
    enter(&mut app);
    assert_row_starts_with(&mut app, "Applying img1.png…");
    assert_no_startup_text(&mut app);
}

#[test]
fn startup_text_never_hides_the_help_line() {
    // Catches: Initializing…/Ready overwriting the help status (shared `status` string), before
    // or after expiry.
    let (mut app, _sys) = fresh("s16-prio-help");
    press(&mut app, '?');
    app.init_started();
    assert_row_starts_with(&mut app, HELP_TEXT);
    assert_no_startup_text(&mut app);
    let t0 = Instant::now();
    app.init_finished(t0);
    assert_row_starts_with(&mut app, HELP_TEXT);
    assert_no_startup_text(&mut app);
    app.tick_at(t0 + MS1500);
    assert_row_starts_with(&mut app, HELP_TEXT);
    assert_no_startup_text(&mut app);
}

#[test]
fn startup_text_never_hides_the_job_status_and_never_replaces_applied() {
    // Catches: Initializing…/Ready overwriting `Applying …`, or `Ready` showing instead of
    // `Applied …` when the job finishes while Ready is still within its 1 s.
    let sys = FakeSystem::new();
    sys.set_job_state(JobState::Running);
    let mut app = app_after_keys("s16-prio-job", &sys, &['l']);
    enter(&mut app);
    assert_row_starts_with(&mut app, "Applying img1.png…");
    app.init_started();
    assert_row_starts_with(&mut app, "Applying img1.png…");
    assert_no_startup_text(&mut app);
    let t0 = Instant::now();
    app.init_finished(t0);
    assert_row_starts_with(&mut app, "Applying img1.png…");
    assert_no_startup_text(&mut app);
    sys.set_job_state(JobState::Done);
    app.tick_at(t0 + Duration::from_millis(100));
    assert_row_starts_with(&mut app, "Applied img1.png");
    assert_no_startup_text(&mut app);
}

#[test]
fn startup_text_never_hides_an_open_prompt() {
    // Catches: Initializing…/Ready replacing the prompt line while the `f` prompt is open.
    let sys = FakeSystem::new();
    let mut app = app_after_keys("s16-prio-prompt", &sys, &['l', 'f']);
    assert_row_starts_with(&mut app, "Font name:");
    app.init_started();
    assert_row_starts_with(&mut app, "Font name:");
    assert_nowhere(&mut app, INITIALIZING);
    let t0 = Instant::now();
    app.init_finished(t0);
    assert_row_starts_with(&mut app, "Font name:");
    assert_nowhere(&mut app, READY);
}

#[test]
fn the_monitor_popup_is_an_overlay_that_does_not_touch_the_startup_text() {
    // Catches: the popup (S15) covering/clearing the status row startup text, or the startup text
    // leaking into the popup box. The full `DP-1  1920x1080@60  on` row only fits inside the
    // popup at 80 columns, so seeing it means the popup is open.
    let sys = FakeSystem::new();
    sys.set_stdout(
        "hyprctl",
        r#"[{"name": "eDP-1", "width": 2880, "height": 1800, "refreshRate": 120.000, "disabled": false},
            {"name": "DP-1", "width": 1920, "height": 1080, "refreshRate": 60.000, "disabled": false}]"#,
    );
    let mut app = monitors_app("s16-popup", &sys);
    press(&mut app, 'j');
    enter(&mut app); // opens the popup
    assert_app_status(&mut app, "DP-1  1920x1080@60  on");
    let t0 = Instant::now();
    app.init_started();
    assert_row_starts_with(&mut app, INITIALIZING);
    assert_app_status(&mut app, "DP-1  1920x1080@60  on");
    app.init_finished(t0);
    assert_row_starts_with(&mut app, READY);
    assert_app_status(&mut app, "DP-1  1920x1080@60  on");
    assert_nowhere(&mut app, INITIALIZING);
    app.tick_at(t0 + MS500);
    assert_row_starts_with(&mut app, READY);
    app.tick_at(t0 + MS1500);
    assert_row_empty(&mut app);
    assert_app_status(&mut app, "DP-1  1920x1080@60  on");
}

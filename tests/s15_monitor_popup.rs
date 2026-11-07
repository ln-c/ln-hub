//! S15-monitor-popup: `Enter` on a Monitors row opens a popup with the full row and the switch it
//! will perform; a second `Enter` confirms (same spawn and statuses as S10). `Esc` and the quit key
//! close it, other keys are swallowed, refusal and a running job are shown inside it.
//!
//! At 80 columns the items column interior is 18 cells, so a full row such as
//! `DP-1  1920x1080@60  on` (22 chars) is truncated there: seeing it whole at 80 columns can only
//! mean the popup is up. That is the "popup is open" observable used below. Status assertions
//! search every rendered row (the status bar location is not fixed by the spec).
//!
//! Only `System` is faked; the wallpaper dir is a real temp dir.

mod common;

use common::*;
use ln_hub::{App, JobState};
use ratatui::crossterm::event::KeyCode;

const EDP_ON: &str = "eDP-1  2880x1800@120  on";
const DP_ON: &str = "DP-1  1920x1080@60  on";
const DP_OFF: &str = "DP-1  1920x1080@60  off";
const HELP_TEXT: &str = "h/j/k/l move";
const REFUSAL: &str = "Refusing to disable the last enabled monitor";

fn mon(name: &str, w: u32, h: u32, hz: &str, disabled: bool) -> String {
    format!(
        r#"{{"name": "{name}", "width": {w}, "height": {h}, "refreshRate": {hz}, "disabled": {disabled}}}"#
    )
}

fn edp(disabled: bool) -> String {
    mon("eDP-1", 2880, 1800, "120.000", disabled)
}

fn dp(disabled: bool) -> String {
    mon("DP-1", 1920, 1080, "60.000", disabled)
}

fn hdmi() -> String {
    mon("HDMI-A-1", 1920, 1080, "60.000", false)
}

fn sys_with(mons: &[String]) -> FakeSystem {
    let sys = FakeSystem::new();
    sys.set_stdout("hyprctl", &format!("[{}]", mons.join(",")));
    sys
}

fn spawn(prog: &str, args: &[&str]) -> (String, Vec<String>) {
    (
        prog.to_string(),
        args.iter().map(|s| s.to_string()).collect(),
    )
}

fn esc(app: &mut App) {
    app.handle_key(key(KeyCode::Esc));
}

/// True when some row of the `w` x 24 render contains `text`.
fn shows_at(app: &mut App, w: u16, text: &str) -> bool {
    buffer_lines(&draw_sized(app, w, DRAW_H))
        .iter()
        .any(|l| l.contains(text))
}

/// Panics with a buffer dump unless `text` is on screen at `w` columns.
fn assert_shows_at(app: &mut App, w: u16, text: &str) {
    let buf = draw_sized(app, w, DRAW_H);
    assert!(
        buffer_lines(&buf).iter().any(|l| l.contains(text)),
        "`{text}` not found at {w} cols; buffer:\n{}",
        dump(&buf)
    );
}

/// Panics with a buffer dump if `text` is on screen at `w` columns.
fn assert_absent_at(app: &mut App, w: u16, text: &str) {
    let buf = draw_sized(app, w, DRAW_H);
    assert!(
        !buffer_lines(&buf).iter().any(|l| l.contains(text)),
        "`{text}` must not be shown at {w} cols; buffer:\n{}",
        dump(&buf)
    );
}

/// eDP-1 + DP-1 both on, cursor on DP-1 (second row), items focused.
fn dp_app(name: &str, sys: &FakeSystem) -> App {
    let mut a = monitors_app(name, sys);
    press(&mut a, 'j');
    a
}

#[test]
fn enter_opens_the_popup_with_the_full_row_and_the_switch_and_spawns_nothing() {
    // Catches: Enter still toggling at once (spawn on the first Enter), no popup at 80 or at 120
    // columns (popup only when the row is truncated), or a popup missing the full row / the
    // `DP-1 off` switch line.
    for w in [80u16, 120] {
        let sys = sys_with(&[edp(false), dp(false)]);
        let mut a = dp_app(&format!("s15-open-{w}"), &sys);
        if w == 80 {
            assert_absent_at(&mut a, w, DP_ON); // truncated in the column: precondition
        }
        assert_absent_at(&mut a, w, "DP-1 off");
        enter(&mut a);
        assert!(
            sys.spawn_calls().is_empty(),
            "{w} cols: the first Enter must not spawn: {:?}",
            sys.spawn_calls()
        );
        assert_shows_at(&mut a, w, DP_ON);
        assert_shows_at(&mut a, w, "DP-1 off");
    }
}

#[test]
fn a_disabled_monitor_popup_shows_and_spawns_on() {
    // Catches: the popup (or its confirm) always saying/spawning `off`, or a last-enabled guard
    // that also blocks ENABLING (eDP-1 is the only enabled monitor here).
    let sys = sys_with(&[edp(false), dp(true)]);
    let mut a = dp_app("s15-on", &sys);
    enter(&mut a);
    assert!(sys.spawn_calls().is_empty(), "{:?}", sys.spawn_calls());
    assert_shows_at(&mut a, 80, DP_OFF);
    assert_shows_at(&mut a, 80, "DP-1 on");
    enter(&mut a);
    assert_eq!(
        sys.spawn_calls(),
        vec![spawn("ln-monitorctl", &["DP-1", "on"])]
    );
}

#[test]
fn confirm_spawns_off_closes_the_popup_and_reports_applying_then_applied() {
    // Catches: the confirm not spawning `ln-monitorctl DP-1 off`, the popup staying open after
    // confirming, or the S10 statuses (`Applying DP-1 off…` / `Applied DP-1 off`) being lost.
    let sys = sys_with(&[edp(false), dp(false)]);
    sys.set_job_state(JobState::Running);
    let mut a = dp_app("s15-confirm", &sys);
    enter(&mut a);
    assert!(sys.spawn_calls().is_empty(), "{:?}", sys.spawn_calls());
    assert_shows_at(&mut a, 80, DP_ON);
    enter(&mut a);
    assert_eq!(
        sys.spawn_calls(),
        vec![spawn("ln-monitorctl", &["DP-1", "off"])]
    );
    assert_absent_at(&mut a, 80, DP_ON);
    assert_app_status(&mut a, "Applying DP-1 off…");

    sys.set_job_state(JobState::Done);
    a.tick();
    assert_app_status(&mut a, "Applied DP-1 off");
}

#[test]
fn a_failed_confirm_shows_apply_failed_with_the_reason() {
    // Catches: a failed job started from the popup being reported as success or losing `<reason>`.
    let sys = sys_with(&[edp(false), dp(false)]);
    sys.set_job_state(JobState::Running);
    let mut a = dp_app("s15-failed", &sys);
    enter(&mut a);
    assert!(sys.spawn_calls().is_empty(), "{:?}", sys.spawn_calls());
    enter(&mut a);
    assert_eq!(sys.spawn_calls().len(), 1, "the toggle must be spawned");
    sys.set_job_state(JobState::Failed("exit 1".to_string()));
    a.tick();
    assert_app_status(&mut a, "Apply failed: exit 1");
}

#[test]
fn esc_closes_the_popup_without_spawning() {
    // Catches: Esc ignored by the popup (stays open), or Esc confirming the toggle.
    let sys = sys_with(&[edp(false), dp(false)]);
    let mut a = dp_app("s15-esc", &sys);
    enter(&mut a);
    assert_shows_at(&mut a, 80, DP_ON);
    esc(&mut a);
    assert_absent_at(&mut a, 80, DP_ON);
    assert!(
        sys.spawn_calls().is_empty(),
        "Esc must not spawn: {:?}",
        sys.spawn_calls()
    );
    assert!(!a.should_quit());
}

#[test]
fn the_quit_key_closes_the_popup_without_quitting_and_quits_again_once_closed() {
    // Catches: `q` quitting the app from inside the popup, `q` ignored by the popup, `q`
    // spawning, or quit staying dead after the popup closed.
    let sys = sys_with(&[edp(false), dp(false)]);
    let mut a = dp_app("s15-quit", &sys);
    enter(&mut a);
    assert_shows_at(&mut a, 80, DP_ON);
    press(&mut a, 'q');
    assert!(!a.should_quit(), "q must close the popup, not quit");
    assert_absent_at(&mut a, 80, DP_ON);
    assert!(sys.spawn_calls().is_empty(), "{:?}", sys.spawn_calls());
    press(&mut a, 'q');
    assert!(a.should_quit(), "q must quit once the popup is closed");
}

#[test]
fn other_keys_are_swallowed_while_the_popup_is_open() {
    // Catches: popup keys falling through to the keymap: `j`/`k` moving the cursor, `h` unfocusing
    // the items, `l`/`/` acting, `g` arming the `gg` prefix (eating the next key after the popup
    // closes), `?` replacing the status with the help line.
    for k in ['j', 'k', 'h', 'l', '/', 'g', '?'] {
        let sys = sys_with(&[edp(false), dp(false), hdmi()]);
        let mut a = dp_app(&format!("s15-swallow-{}", k as u32), &sys);
        // Leave a known status behind first.
        enter(&mut a);
        enter(&mut a);
        a.tick();
        assert_app_status(&mut a, "Applied DP-1 off");
        assert_eq!(sys.spawn_calls().len(), 1);

        enter(&mut a);
        assert_shows_at(&mut a, 80, DP_ON);
        press(&mut a, k);
        assert_shows_at(&mut a, 80, DP_ON);
        assert_absent_at(&mut a, 80, HELP_TEXT);
        esc(&mut a);
        assert_absent_at(&mut a, 80, DP_ON);
        assert_absent_at(&mut a, 80, HELP_TEXT);
        assert_app_status(&mut a, "Applied DP-1 off");

        // Cursor still on DP-1 and the items still focused: Enter, Enter spawns DP-1 again.
        enter(&mut a);
        enter(&mut a);
        assert_eq!(
            sys.spawn_calls(),
            vec![
                spawn("ln-monitorctl", &["DP-1", "off"]),
                spawn("ln-monitorctl", &["DP-1", "off"]),
            ],
            "key `{k}` leaked through the popup"
        );
    }
}

#[test]
fn refusal_popup_shows_the_row_and_the_reason_and_enter_does_nothing() {
    // Catches: no popup for the refusal (status only), the popup missing the full row or the
    // refusal text, Enter in it spawning (disabling the last output), or Enter in it leaving a job
    // in flight (the next toggle would be swallowed). Esc and the quit key close it.
    for closer in ['e', 'q'] {
        let sys = sys_with(&[edp(false), dp(true)]);
        let mut a = monitors_app(&format!("s15-refusal-{closer}"), &sys);
        enter(&mut a);
        assert_shows_at(&mut a, 80, EDP_ON);
        assert_shows_at(&mut a, 80, REFUSAL);
        enter(&mut a);
        assert!(
            sys.spawn_calls().is_empty(),
            "Enter in the refusal popup must not spawn: {:?}",
            sys.spawn_calls()
        );

        if closer == 'e' {
            esc(&mut a);
        } else {
            press(&mut a, 'q');
            assert!(!a.should_quit(), "q must close the refusal popup, not quit");
        }
        assert_absent_at(&mut a, 80, EDP_ON);
        assert_absent_at(&mut a, 80, REFUSAL);

        // No job in flight: enabling DP-1 through the popup works at once.
        press(&mut a, 'j');
        enter(&mut a);
        enter(&mut a);
        assert_eq!(
            sys.spawn_calls(),
            vec![spawn("ln-monitorctl", &["DP-1", "on"])],
            "closer `{closer}`"
        );
    }
}

#[test]
fn refusal_popup_opens_at_120_columns_too() {
    // Catches: the refusal popup only built when the row is truncated (80 columns): at 120 the
    // refusal text must still be shown (it is not in the column, only in the popup), and it must
    // not outlive the popup as a status line.
    let sys = sys_with(&[edp(false), dp(true)]);
    let mut a = monitors_app("s15-refusal-120", &sys);
    assert_absent_at(&mut a, 120, REFUSAL);
    enter(&mut a);
    assert_shows_at(&mut a, 120, REFUSAL);
    assert!(sys.spawn_calls().is_empty(), "{:?}", sys.spawn_calls());
    // The reason belongs to the popup (not the status): it goes away with it.
    esc(&mut a);
    assert_absent_at(&mut a, 120, REFUSAL);
}

#[test]
fn popup_while_a_job_runs_shows_applying_and_enter_is_silent_and_esc_closes() {
    // Catches: opening a confirmable popup while a toggle job is in flight (duplicate
    // ln-monitorctl job on Enter), no `Applying…` notice in the popup, or Esc not closing it.
    for closer_is_esc in [false, true] {
        let sys = sys_with(&[edp(false), dp(false)]);
        sys.set_job_state(JobState::Running);
        let mut a = dp_app(&format!("s15-running-{closer_is_esc}"), &sys);
        enter(&mut a);
        enter(&mut a);
        assert_eq!(sys.spawn_calls().len(), 1, "first toggle must be spawned");
        a.tick();
        assert_absent_at(&mut a, 80, "Applying…"); // the status says `Applying DP-1 off…`

        enter(&mut a);
        assert_shows_at(&mut a, 80, "Applying…");
        if closer_is_esc {
            esc(&mut a);
            assert_absent_at(&mut a, 80, "Applying…");
        } else {
            enter(&mut a);
            esc(&mut a);
        }
        assert_eq!(
            sys.spawn_calls().len(),
            1,
            "no second job while one runs: {:?}",
            sys.spawn_calls()
        );
    }
}

#[test]
fn preview_image_is_none_while_the_popup_is_open_also_after_a_render() {
    // Regression guard (passes before S15 because Monitors never carry a preview image): the bin
    // paints the sixel after `render`, which would cover the popup, so the image must stay None
    // while the popup is open, before and after a render, at both widths.
    for w in [80u16, 120] {
        let sys = sys_with(&[edp(false), dp(false)]);
        let mut a = dp_app(&format!("s15-preview-{w}"), &sys);
        enter(&mut a);
        assert!(a.preview_image().is_none());
        let _ = draw_sized(&mut a, w, DRAW_H);
        assert!(a.preview_image().is_none(), "{w} cols: after render");
        assert!(shows_at(&mut a, w, "DP-1 off"), "popup must be open");
    }
}

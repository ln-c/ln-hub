//! S10-monitor-toggle: `Enter` on a Monitors row toggles that monitor through `ln-monitorctl`,
//! refuses to disable the last enabled monitor, and refreshes the list after the job.
//!
//! Since S15 the toggle goes through a popup: the first `Enter` opens it, a second `Enter` confirms
//! (helper `confirm`). The popup's own behavior is pinned in `s15_monitor_popup.rs`; these tests keep
//! their S10 intent (spawn args, refusal, re-query, statuses, error wording) behind `Enter`, `Enter`.
//!
//! Rendered on a 120x24 buffer (as in S9): the items column is x 21..53. Status assertions search
//! every rendered row, because the spec does not fix the status bar location.
//!
//! Highlight observable (as in S5/S6): the "style" of an items row is the triple (fg, bg, modifier)
//! of the cell holding the FIRST letter of the monitor row inside the items column. With three
//! monitors, the highlighted row is the one whose style differs from the other two, which share one.
//!
//! Only `System` is faked; the wallpaper dir is a real temp dir.

mod common;

use std::io::ErrorKind;

use common::*;
use ln_hub::{App, Config, JobState, Module};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;

const W: u16 = 120;
const H: u16 = 24;
const ITEMS_X_START: u16 = 21;
const ITEMS_X_END: u16 = 53;

const EDP_ON: &str = "eDP-1  2880x1800@120  on";
const DP_ON: &str = "DP-1  2560x1440@60  on";
const DP_OFF: &str = "DP-1  2560x1440@60  off";
const HDMI_ON: &str = "HDMI-A-1  1920x1080@60  on";

fn mon(name: &str, w: u32, h: u32, hz: &str, disabled: bool) -> String {
    format!(
        r#"{{"name": "{name}", "width": {w}, "height": {h}, "refreshRate": {hz}, "disabled": {disabled}}}"#
    )
}

fn edp(disabled: bool) -> String {
    mon("eDP-1", 2880, 1800, "120.000", disabled)
}

fn dp(disabled: bool) -> String {
    mon("DP-1", 2560, 1440, "60.000", disabled)
}

fn hdmi(disabled: bool) -> String {
    mon("HDMI-A-1", 1920, 1080, "60.000", disabled)
}

/// `hyprctl monitors all -j` output listing `mons` in order.
fn json(mons: &[String]) -> String {
    format!("[{}]", mons.join(","))
}

fn sys_with(mons: &[String]) -> FakeSystem {
    let sys = FakeSystem::new();
    sys.set_stdout("hyprctl", &json(mons));
    sys
}

fn draw(app: &mut App) -> Buffer {
    draw_sized(app, W, H)
}

fn shows(app: &mut App, text: &str) -> bool {
    buffer_lines(&draw(app)).iter().any(|l| l.contains(text))
}

/// Asserts some rendered row contains `text`, at this file's wide size (not the shared 80x24
/// `assert_app_status`).
fn assert_status(app: &mut App, text: &str) {
    common::assert_status(&draw(app), text);
}

fn key_esc(app: &mut App) {
    app.handle_key(key(KeyCode::Esc));
}

/// Opens the monitor popup and confirms it (S15): the pair of `Enter`s that used to be one. The
/// first `Enter` must only open the popup, so it must not spawn anything.
fn confirm(app: &mut App, sys: &FakeSystem) {
    let before = sys.spawn_calls().len();
    enter(app);
    assert_eq!(
        sys.spawn_calls().len(),
        before,
        "the first Enter must only open the popup, not spawn"
    );
    enter(app);
}

/// Asserts the refusal popup is up: at 80 columns the items column truncates `EDP_ON`, so the full
/// row can only come from the popup; the refusal text is part of the popup too (S15).
fn assert_refusal_popup(app: &mut App) {
    let buf = draw_sized(app, 80, H);
    let lines = buffer_lines(&buf);
    assert!(
        lines.iter().any(|l| l.contains(EDP_ON))
            && lines
                .iter()
                .any(|l| l.contains("Refusing to disable the last enabled monitor")),
        "refusal popup (full row + refusal text) not shown; buffer:\n{}",
        dump(&buf)
    );
}

fn spawn(prog: &str, args: &[&str]) -> (String, Vec<String>) {
    (
        prog.to_string(),
        args.iter().map(|s| s.to_string()).collect(),
    )
}

/// First cell of `text` lying entirely inside the items column (this file's x-range differs from
/// s11's, so the wrapper stays local).
fn items_hit(buf: &Buffer, text: &str) -> Option<(u16, u16)> {
    items_hit_within(buf, text, ITEMS_X_START, ITEMS_X_END)
}

/// Asserts `expected` is the only highlighted row among `rows`.
fn assert_only_highlighted_among(app: &mut App, rows: &[&str], expected: &str) {
    let buf = draw(app);
    common::assert_only_highlighted(&buf, rows, expected, items_hit);
}

/// Asserts `expected` is the only highlighted row among eDP-1, DP-1 and HDMI-A-1.
fn assert_only_highlighted(app: &mut App, expected: &str) {
    assert_only_highlighted_among(app, &[EDP_ON, DP_ON, HDMI_ON], expected);
}

#[test]
fn enter_on_an_enabled_monitor_spawns_monitorctl_off() {
    // Catches: Enter doing nothing on Monitors, or spawning the wrong program/args (first row
    // instead of the cursor row, `on` instead of `off`, wrong argument order).
    let sys = sys_with(&[edp(false), dp(false)]);
    let mut a = monitors_app("s10-off", &sys);
    press(&mut a, 'j');
    confirm(&mut a, &sys);
    assert_eq!(
        sys.spawn_calls(),
        vec![spawn("ln-monitorctl", &["DP-1", "off"])]
    );
}

#[test]
fn enter_on_a_disabled_monitor_spawns_monitorctl_on() {
    // Catches: always passing `off`, and a last-enabled guard that also blocks ENABLING a monitor
    // when only one is enabled (here eDP-1 is the only enabled one and DP-1 is being enabled).
    let sys = sys_with(&[edp(false), dp(true)]);
    let mut a = monitors_app("s10-on", &sys);
    press(&mut a, 'j');
    confirm(&mut a, &sys);
    assert_eq!(
        sys.spawn_calls(),
        vec![spawn("ln-monitorctl", &["DP-1", "on"])]
    );
}

#[test]
fn disabling_the_last_enabled_monitor_is_refused_with_a_popup() {
    // Catches: disabling the only enabled monitor (leaving no output), or refusing silently
    // without the `Refusing to disable the last enabled monitor` popup.
    let sys = sys_with(&[edp(false), dp(true)]);
    let mut a = monitors_app("s10-last", &sys);
    enter(&mut a);
    assert_refusal_popup(&mut a);
    enter(&mut a);
    assert!(
        sys.spawn_calls().is_empty(),
        "must not spawn: {:?}",
        sys.spawn_calls()
    );
}

#[test]
fn done_job_requeries_hyprctl_and_shows_the_refreshed_rows() {
    // Catches: no list refresh after the toggle job (stale `on` row), refreshing while the job is
    // still Running, or refreshing without actually showing the new hyprctl output.
    let sys = sys_with(&[edp(false), dp(false)]);
    let mut a = monitors_app("s10-refresh", &sys);
    assert!(items_hit(&draw(&mut a), DP_ON).is_some());
    press(&mut a, 'j');
    sys.set_job_state(JobState::Running);
    confirm(&mut a, &sys);
    let before = hyprctl_runs(&sys);

    a.tick();
    assert_eq!(
        hyprctl_runs(&sys),
        before,
        "no refresh while the job is Running"
    );

    sys.set_stdout("hyprctl", &json(&[edp(false), dp(true)]));
    sys.set_job_state(JobState::Done);
    a.tick();
    assert_eq!(
        hyprctl_runs(&sys),
        before + 1,
        "Done must re-run `hyprctl monitors all -j` exactly once"
    );
    let buf = draw(&mut a);
    assert!(
        items_hit(&buf, DP_OFF).is_some() && items_hit(&buf, DP_ON).is_none(),
        "refreshed rows not shown; buffer:\n{}",
        dump(&buf)
    );
}

#[test]
fn cursor_starts_on_the_first_monitor_moves_with_j_k_and_clamps() {
    // Catches: no monitor cursor (no row highlighted), the cursor not starting on the first
    // monitor, `j`/`k` not moving it, or running past either end.
    let sys = sys_with(&[edp(false), dp(false), hdmi(false)]);
    let mut a = monitors_app("s10-cursor", &sys);
    assert_only_highlighted(&mut a, EDP_ON);
    press(&mut a, 'j');
    assert_only_highlighted(&mut a, DP_ON);
    press(&mut a, 'j');
    assert_only_highlighted(&mut a, HDMI_ON);
    for _ in 0..3 {
        press(&mut a, 'j');
    }
    assert_only_highlighted(&mut a, HDMI_ON);
    press(&mut a, 'k');
    assert_only_highlighted(&mut a, DP_ON);
    for _ in 0..4 {
        press(&mut a, 'k');
    }
    assert_only_highlighted(&mut a, EDP_ON);
}

#[test]
fn j_inside_monitors_items_does_not_move_the_module_column() {
    // Catches: `j` in the Monitors items still driving the module column instead of the monitor
    // cursor. Monitors is listed FIRST here, so a buggy `j` would move the module selection to
    // Theme (swapping the view to wallpapers); the Monitors view must stay up and Enter must act
    // on the moved cursor row.
    let sys = sys_with(&[edp(false), dp(false), hdmi(false)]);
    let mut a = app_with_modules(
        "s10-nomodule",
        &sys,
        Some(vec![Module::Monitors, Module::Theme]),
        &['l'],
    );
    assert_only_highlighted(&mut a, EDP_ON);
    press(&mut a, 'j');
    press(&mut a, 'j');
    let buf = draw(&mut a);
    assert!(
        items_hit(&buf, HDMI_ON).is_some() && !shows(&mut a, "img1.png"),
        "Monitors view must stay up; buffer:\n{}",
        dump(&buf)
    );
    assert_only_highlighted(&mut a, HDMI_ON);
    confirm(&mut a, &sys);
    assert_eq!(
        sys.spawn_calls(),
        vec![spawn("ln-monitorctl", &["HDMI-A-1", "off"])]
    );
}

#[test]
fn cursor_clamps_after_a_refresh_that_shrinks_the_list() {
    // Catches: the cursor staying past the end after the refreshed list loses a monitor (no
    // highlighted row, and Enter toggling a non-existent row / doing nothing).
    let sys = sys_with(&[edp(false), dp(false), hdmi(false)]);
    let mut a = monitors_app("s10-shrink", &sys);
    press(&mut a, 'j');
    press(&mut a, 'j');
    confirm(&mut a, &sys);
    assert_eq!(
        sys.spawn_calls(),
        vec![spawn("ln-monitorctl", &["HDMI-A-1", "off"])]
    );

    sys.set_stdout("hyprctl", &json(&[edp(false), dp(false)]));
    a.tick();
    let buf = draw(&mut a);
    assert!(
        items_hit(&buf, HDMI_ON).is_none(),
        "HDMI-A-1 must be gone after the refresh; buffer:\n{}",
        dump(&buf)
    );
    assert_only_highlighted_among(&mut a, &[EDP_ON, DP_ON], DP_ON);
    confirm(&mut a, &sys);
    assert_eq!(
        sys.spawn_calls(),
        vec![
            spawn("ln-monitorctl", &["HDMI-A-1", "off"]),
            spawn("ln-monitorctl", &["DP-1", "off"]),
        ]
    );
}

#[test]
fn a_refusal_leaves_no_job_in_flight() {
    // Catches: the last-enabled refusal marking a job as in flight (without spawning), so the
    // next Enter on another monitor is swallowed until a tick that never comes.
    let sys = sys_with(&[edp(false), dp(true)]);
    let mut a = monitors_app("s10-refusal-free", &sys);
    enter(&mut a);
    assert_refusal_popup(&mut a);
    assert!(sys.spawn_calls().is_empty());

    key_esc(&mut a);
    press(&mut a, 'j');
    confirm(&mut a, &sys);
    assert_eq!(
        sys.spawn_calls(),
        vec![spawn("ln-monitorctl", &["DP-1", "on"])]
    );
}

#[test]
fn enter_on_a_disabled_first_monitor_spawns_on_without_moving() {
    // Catches: the toggle direction or cursor row being derived from something other than the
    // initial cursor (first monitor) when that monitor is disabled.
    let sys = sys_with(&[edp(true), dp(false)]);
    let mut a = monitors_app("s10-first-disabled", &sys);
    confirm(&mut a, &sys);
    assert_eq!(
        sys.spawn_calls(),
        vec![spawn("ln-monitorctl", &["eDP-1", "on"])]
    );
}

#[test]
fn spawning_shows_applying_and_done_shows_applied() {
    // Catches: no status while the job runs (`Applying DP-1 off…` with the single-char `…`), or
    // the status stuck on `Applying` / not naming the monitor and action after Done.
    let sys = sys_with(&[edp(false), dp(false)]);
    sys.set_job_state(JobState::Running);
    let mut a = monitors_app("s10-status", &sys);
    press(&mut a, 'j');
    confirm(&mut a, &sys);
    assert_status(&mut a, "Applying DP-1 off…");

    sys.set_job_state(JobState::Done);
    a.tick();
    assert_status(&mut a, "Applied DP-1 off");
}

#[test]
fn failed_job_shows_apply_failed_and_does_not_requery_hyprctl() {
    // Catches: a failed toggle reported as success, the reason (`exit 1`) dropped, or the list
    // being refreshed after a failure (nothing changed, so hyprctl must not be re-run).
    let sys = sys_with(&[edp(false), dp(false)]);
    sys.set_job_state(JobState::Running);
    let mut a = monitors_app("s10-failed", &sys);
    press(&mut a, 'j');
    confirm(&mut a, &sys);
    assert_eq!(sys.spawn_calls().len(), 1, "the toggle must be spawned");
    let before = hyprctl_runs(&sys);

    sys.set_job_state(JobState::Failed("exit 1".to_string()));
    a.tick();
    assert_status(&mut a, "Apply failed: exit 1");
    assert_eq!(hyprctl_runs(&sys), before, "Failed must not re-run hyprctl");
}

#[test]
fn second_enter_while_running_does_not_spawn_again_but_works_after_done() {
    // Catches: no in-flight guard for the toggle (duplicate ln-monitorctl job), or a guard never
    // released once the job is Done (Enter dead forever).
    let sys = sys_with(&[edp(false), dp(false)]);
    sys.set_job_state(JobState::Running);
    let mut a = monitors_app("s10-twice", &sys);
    press(&mut a, 'j');
    confirm(&mut a, &sys);
    a.tick();
    enter(&mut a);
    enter(&mut a);
    key_esc(&mut a);
    assert_eq!(sys.spawn_calls().len(), 1, "calls: {:?}", sys.spawn_calls());

    sys.set_job_state(JobState::Done);
    a.tick();
    confirm(&mut a, &sys);
    assert_eq!(sys.spawn_calls().len(), 2, "calls: {:?}", sys.spawn_calls());
}

#[test]
fn monitorctl_spawn_error_wording_depends_on_the_error_kind() {
    // Catches: swallowing the io::Error or panicking, `not found` wording for any error kind, and
    // the program name missing from the message.
    let sys = sys_with(&[edp(false), dp(false)]);
    sys.set_spawn_error(ErrorKind::NotFound, "not found");
    let mut a = monitors_app("s10-spawn-nf", &sys);
    press(&mut a, 'j');
    confirm(&mut a, &sys);
    assert_status(&mut a, "ln-monitorctl not found: not found");

    let sys = sys_with(&[edp(false), dp(false)]);
    sys.set_spawn_error(ErrorKind::PermissionDenied, "denied");
    let mut a = monitors_app("s10-spawn-pd", &sys);
    press(&mut a, 'j');
    confirm(&mut a, &sys);
    assert_status(&mut a, "Failed to start ln-monitorctl: denied");
    assert!(
        !shows(&mut a, "not found"),
        "a PermissionDenied error must not be reported as `not found`"
    );
}

#[test]
fn themectl_spawn_error_wording_depends_on_the_error_kind() {
    // Catches: the `ln-themectl not found: …` wording being used for errors other than NotFound
    // (S6/S8 carried note); the NotFound wording itself is pinned in s6.
    let sys = FakeSystem::new();
    sys.set_spawn_error(ErrorKind::PermissionDenied, "denied");
    let mut a = app_after_keys("s10-themectl-pd", &sys, &['l']);
    enter(&mut a);
    assert_eq!(sys.spawn_calls().len(), 1, "a spawn must be attempted");
    assert_status(&mut a, "Failed to start ln-themectl: denied");
    assert!(
        !shows(&mut a, "not found"),
        "a PermissionDenied error must not be reported as `not found`"
    );
}

#[test]
fn enter_with_items_unfocused_spawns_nothing() {
    // Catches: Enter acting from the module column (applying the first wallpaper on Theme, or
    // toggling a monitor on Monitors before `l` focused the items).
    let sys = sys_with(&[edp(false), dp(false)]);
    let mut theme = app_after_keys("s10-unfocused-theme", &sys, &[]);
    enter(&mut theme);
    let mut monitors = app_after_keys("s10-unfocused-mon", &sys, &['j']);
    enter(&mut monitors);
    assert!(
        sys.spawn_calls().is_empty(),
        "no spawn from the module column: {:?}",
        sys.spawn_calls()
    );
}

#[test]
fn enter_on_an_empty_wallpaper_dir_spawns_nothing_and_does_not_panic() {
    // Catches: unwrapping the (absent) selected wallpaper, or spawning ln-themectl with no file.
    let sys = FakeSystem::new();
    let home = tempdir("s10-empty-home");
    let dir = tempdir("s10-empty-wall");
    let mut cfg = Config::default_for(&home);
    cfg.wallpaper_dir = dir;
    let mut a = App::new(cfg, Box::new(sys.clone()));
    press(&mut a, 'l');
    enter(&mut a);
    draw(&mut a);
    assert!(
        sys.spawn_calls().is_empty(),
        "no spawn without wallpapers: {:?}",
        sys.spawn_calls()
    );
}

#[test]
fn enter_on_monitors_without_a_usable_list_spawns_nothing() {
    // Catches: unwrapping the cursor row when hyprctl failed / printed garbage / listed nothing,
    // or toggling a monitor that is not in the list.
    for (label, setup) in [("run-error", 0), ("garbage", 1), ("empty-list", 2)] {
        let sys = FakeSystem::new();
        match setup {
            0 => sys.set_run_error("hyprctl", ErrorKind::NotFound, "No such file or directory"),
            1 => sys.set_stdout("hyprctl", "not json"),
            _ => sys.set_stdout("hyprctl", "[]"),
        }
        let mut a = monitors_app(&format!("s10-unusable-{label}"), &sys);
        enter(&mut a);
        draw(&mut a);
        assert!(
            sys.spawn_calls().is_empty(),
            "{label}: no spawn without a monitor list: {:?}",
            sys.spawn_calls()
        );
    }
}

//! S9-monitor-list: the Monitors module lists monitors from `hyprctl monitors all -j`.
//!
//! Rendered on a 120x24 buffer: at 80 columns the items column interior is 18 cells, too narrow
//! for `eDP-1  2880x1800@120  on` (24 chars). At 120 columns the modules column is x 0..20 and the
//! items column starts at x 20 and is ~33 wide, so rows are checked inside it (x 21..53).
//! Only `System` is faked; the wallpaper dir is a real temp dir.

mod common;

use std::io;

use common::*;
use ln_hub::{App, JobState};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;

const W: u16 = 120;
const H: u16 = 24;
const ITEMS_X_START: u16 = 21; // first interior cell of the items column
const ITEMS_X_END: u16 = 53; // exclusive: the items column ends at or before this
const DEFAULT_FONT: &str = "Terminess Nerd Font Propo";
const UNAVAILABLE: &str = "hyprctl unavailable";

const EDP_ROW: &str = "eDP-1  2880x1800@120  on";
const DP_ROW: &str = "DP-1  2560x1440@60  off";

/// Realistic `hyprctl monitors all -j` output: floats for `refreshRate`, many fields the list
/// must ignore, and `name` at different positions in each object.
const MONITORS_JSON: &str = r#"[
  {
    "id": 0,
    "name": "eDP-1",
    "description": "BOE 0x0BCA",
    "make": "BOE",
    "model": "0x0BCA",
    "serial": "",
    "width": 2880,
    "height": 1800,
    "physicalWidth": 300,
    "physicalHeight": 190,
    "refreshRate": 120.000,
    "x": 0,
    "y": 0,
    "activeWorkspace": {"id": 1, "name": "1"},
    "reserved": [0, 0, 0, 0],
    "scale": 1.50,
    "transform": 0,
    "focused": true,
    "dpmsStatus": true,
    "vrr": false,
    "disabled": false,
    "availableModes": ["2880x1800@120.00Hz", "2880x1800@60.00Hz"]
  },
  {
    "id": 1,
    "description": "Dell Inc. DELL U2723QE",
    "make": "Dell Inc.",
    "width": 2560,
    "height": 1440,
    "refreshRate": 60.000,
    "x": 2880,
    "y": 0,
    "scale": 1.00,
    "focused": false,
    "disabled": true,
    "name": "DP-1",
    "availableModes": ["2560x1440@60.00Hz"]
  }
]"#;

fn hyprctl_call() -> (String, Vec<String>) {
    (
        "hyprctl".to_string(),
        ["monitors", "all", "-j"]
            .iter()
            .map(|s| s.to_string())
            .collect(),
    )
}

fn draw(app: &mut App) -> Buffer {
    let mut term = Terminal::new(TestBackend::new(W, H)).unwrap();
    term.draw(|f| app.render(f)).unwrap();
    term.backend().buffer().clone()
}

/// The row `y` where `text` starts inside the items column, if it is drawn there.
fn row_in_items(buf: &Buffer, text: &str) -> Option<u16> {
    items_hit_within(buf, text, ITEMS_X_START, ITEMS_X_END).map(|(_, y)| y)
}

#[test]
fn monitors_are_listed_with_resolution_refresh_and_state() {
    // Catches: no monitor list, a wrong `hyprctl` parse (refreshRate float not rounded to 120/60,
    // `disabled` not mapped to on/off, extra JSON fields breaking parsing), wrong field spacing,
    // or rows rendered outside the items column.
    let sys = FakeSystem::new();
    sys.set_stdout("hyprctl", MONITORS_JSON);
    let mut a = monitors_app("s9-list", &sys);
    let buf = draw(&mut a);
    let edp = row_in_items(&buf, EDP_ROW);
    let dp = row_in_items(&buf, DP_ROW);
    assert!(
        edp.is_some() && dp.is_some(),
        "`{EDP_ROW}` / `{DP_ROW}` not in the items column; buffer:\n{}",
        dump(&buf)
    );
    assert!(edp < dp, "eDP-1 must be listed above DP-1 (JSON order)");
}

#[test]
fn monitors_are_read_with_hyprctl_monitors_all_json() {
    // Catches: reading monitors with the wrong command (`hyprctl monitors -j` omits disabled
    // monitors; `-j` before `monitors`) or never querying hyprctl at all.
    let sys = FakeSystem::new();
    sys.set_stdout("hyprctl", MONITORS_JSON);
    let mut a = monitors_app("s9-cmd", &sys);
    draw(&mut a);
    let hypr: Vec<_> = sys
        .run_calls()
        .into_iter()
        .filter(|(prog, _)| prog == "hyprctl")
        .collect();
    assert!(!hypr.is_empty(), "hyprctl was never run");
    assert!(
        hypr.iter().all(|c| *c == hyprctl_call()),
        "every hyprctl call must be `hyprctl monitors all -j`, got {hypr:?}"
    );
}

#[test]
fn monitor_values_come_from_the_json_not_from_fixed_text() {
    // Catches: a hard-coded list matching the spec example, or a state/refresh mapping that
    // ignores `disabled` / `refreshRate` (here the first monitor is off and the second on).
    let json = r#"[
      {"name": "HDMI-A-1", "width": 1920, "height": 1080, "refreshRate": 59.940, "disabled": true},
      {"name": "DP-2", "width": 3840, "height": 2160, "refreshRate": 144.000, "disabled": false}
    ]"#;
    let sys = FakeSystem::new();
    sys.set_stdout("hyprctl", json);
    let mut a = monitors_app("s9-values", &sys);
    let buf = draw(&mut a);
    let hdmi = row_in_items(&buf, "HDMI-A-1  1920x1080@60  off");
    let dp = row_in_items(&buf, "DP-2  3840x2160@144  on");
    assert!(
        hdmi.is_some() && dp.is_some(),
        "rows from the JSON not shown; buffer:\n{}",
        dump(&buf)
    );
    assert!(
        row_in_items(&buf, EDP_ROW).is_none(),
        "spec example rows must not appear for other JSON"
    );
}

#[test]
fn hyprctl_failure_shows_unavailable_in_the_items_column_without_panicking() {
    // Catches: unwrapping the `run` result (panic), or a failure that leaves the items column
    // blank / outside the items column instead of `hyprctl unavailable`.
    let sys = FakeSystem::new();
    sys.set_run_error(
        "hyprctl",
        io::ErrorKind::NotFound,
        "No such file or directory",
    );
    let mut a = monitors_app("s9-err", &sys);
    let buf = draw(&mut a);
    assert!(
        row_in_items(&buf, UNAVAILABLE).is_some(),
        "`{UNAVAILABLE}` not in the items column; buffer:\n{}",
        dump(&buf)
    );
    assert!(row_in_items(&buf, EDP_ROW).is_none());
}

#[test]
fn unavailable_message_is_absent_when_hyprctl_works() {
    // Catches: always showing `hyprctl unavailable` (even on success), alongside the monitor rows.
    let sys = FakeSystem::new();
    sys.set_stdout("hyprctl", MONITORS_JSON);
    let mut a = monitors_app("s9-ok", &sys);
    let buf = draw(&mut a);
    assert!(
        !buffer_lines(&buf).iter().any(|l| l.contains(UNAVAILABLE)),
        "`{UNAVAILABLE}` must not be shown on success; buffer:\n{}",
        dump(&buf)
    );
}

#[test]
fn monitors_view_shows_neither_the_font_line_nor_the_wallpaper_list() {
    // Catches: Theme-only content (font name/size summary, wallpaper rows) leaking into the
    // Monitors view when its items column is focused (S8 note: font line unpinned for non-Theme).
    let sys = FakeSystem::new();
    sys.set_stdout("hyprctl", MONITORS_JSON);
    let mut a = monitors_app("s9-no-theme", &sys);
    let buf = draw(&mut a);
    let lines = buffer_lines(&buf);
    for text in [DEFAULT_FONT, "img1.png", "img2.png"] {
        assert!(
            !lines.iter().any(|l| l.contains(text)),
            "`{text}` must not be shown in the Monitors view; buffer:\n{}",
            dump(&buf)
        );
    }
}

fn shows(app: &mut App, text: &str) -> bool {
    buffer_lines(&draw(app)).iter().any(|l| l.contains(text))
}

#[test]
fn open_prompt_survives_a_job_finishing_and_status_shows_after_esc() {
    // Catches: tick closing/clobbering an open prompt (or typed text) when the in-flight font job
    // finishes, or the "Applied ..." status being lost instead of shown once the prompt closes.
    let sys = FakeSystem::new();
    let mut a = app_after_keys("s9-prompt-tick", &sys, &['l']); // Theme items focused
    sys.set_job_state(JobState::Running);
    for c in ['s', '1', '8'] {
        a.handle_key(key(KeyCode::Char(c)));
    }
    a.handle_key(key(KeyCode::Enter));
    assert_eq!(sys.spawn_calls().len(), 1, "font size job must be spawned");

    a.handle_key(key(KeyCode::Char('f')));
    for c in "Zed".chars() {
        a.handle_key(key(KeyCode::Char(c)));
    }
    sys.set_job_state(JobState::Done);
    a.tick();
    let buf = draw(&mut a);
    assert!(
        buffer_lines(&buf)
            .iter()
            .any(|l| l.contains("Font name: Zed")),
        "prompt `Font name: Zed` must stay open after tick; buffer:\n{}",
        dump(&buf)
    );

    a.handle_key(key(KeyCode::Esc));
    let buf = draw(&mut a);
    assert!(
        buffer_lines(&buf)
            .iter()
            .any(|l| l.contains("Applied font size 18")),
        "`Applied font size 18` must show after Esc; buffer:\n{}",
        dump(&buf)
    );
    assert!(!buffer_lines(&buf).iter().any(|l| l.contains("Font name:")));
}

#[test]
fn unparseable_hyprctl_output_shows_unavailable() {
    // Catches: treating non-JSON stdout as an empty list (blank column) or panicking on it.
    let sys = FakeSystem::new();
    sys.set_stdout("hyprctl", "not json");
    let mut a = monitors_app("s9-garbage", &sys);
    let buf = draw(&mut a);
    assert!(
        row_in_items(&buf, UNAVAILABLE).is_some(),
        "`{UNAVAILABLE}` not in the items column; buffer:\n{}",
        dump(&buf)
    );
}

#[test]
fn hyprctl_is_not_run_before_l_on_monitors_nor_when_l_focuses_theme() {
    // Catches: querying hyprctl eagerly (in App::new, on `j` selection, or on render) or when
    // `l` focuses a module other than Monitors.
    let sys = FakeSystem::new();
    sys.set_stdout("hyprctl", MONITORS_JSON);

    let mut on_monitors = app_after_keys("s9-lazy-mon", &sys, &['j']);
    draw(&mut on_monitors);
    assert_eq!(hyprctl_runs(&sys), 0, "no hyprctl before `l` on Monitors");
    on_monitors.handle_key(key(KeyCode::Char('l')));
    assert_eq!(
        hyprctl_runs(&sys),
        1,
        "`l` on Monitors queries exactly once"
    );

    let sys = FakeSystem::new();
    sys.set_stdout("hyprctl", MONITORS_JSON);
    let mut on_theme = app_after_keys("s9-lazy-theme", &sys, &['l']);
    draw(&mut on_theme);
    assert_eq!(hyprctl_runs(&sys), 0, "`l` on Theme must not run hyprctl");
}

#[test]
fn pressing_l_again_on_monitors_requeries_hyprctl() {
    // Catches: caching the first monitor list (second `l` not refreshing after a hotplug).
    let sys = FakeSystem::new();
    sys.set_stdout("hyprctl", MONITORS_JSON);
    let mut a = monitors_app("s9-requery", &sys);
    assert!(row_in_items(&draw(&mut a), EDP_ROW).is_some());

    sys.set_stdout(
        "hyprctl",
        r#"[{"name": "HDMI-A-1", "width": 1920, "height": 1080, "refreshRate": 60.0, "disabled": false}]"#,
    );
    a.handle_key(key(KeyCode::Char('l')));
    let buf = draw(&mut a);
    assert!(
        row_in_items(&buf, "HDMI-A-1  1920x1080@60  on").is_some(),
        "new monitor row missing after second `l`; buffer:\n{}",
        dump(&buf)
    );
    assert!(
        row_in_items(&buf, EDP_ROW).is_none(),
        "stale row still shown; buffer:\n{}",
        dump(&buf)
    );
}

#[test]
fn f_and_s_do_nothing_on_monitors() {
    // Catches: Theme-only keys (font prompts) firing while Monitors items are focused, e.g. a
    // guard on `items_focused` alone instead of "Theme items focused". (`Enter` on Monitors is the
    // toggle, pinned in S10, including that it never spawns `ln-themectl`.)
    for k in [KeyCode::Char('f'), KeyCode::Char('s')] {
        let sys = FakeSystem::new();
        sys.set_stdout("hyprctl", MONITORS_JSON);
        let mut a = monitors_app("s9-theme-keys", &sys);
        a.handle_key(key(k));
        assert!(
            !shows(&mut a, "Font name:") && !shows(&mut a, "Font size:"),
            "{k:?} must not open a prompt on Monitors"
        );
        assert!(
            sys.spawn_calls().is_empty(),
            "{k:?} must not spawn on Monitors: {:?}",
            sys.spawn_calls()
        );
    }
}

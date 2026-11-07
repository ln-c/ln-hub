//! S13-keys (R3): `gg` / `G` jump to the first / last row of the focused column, a lone `g`
//! swallows the next key, `?` shows the key help in the status row, and `g` / `G` / `?` are
//! reserved (not allowed as the quit key) but plain text inside prompts.
//!
//! Observables: the highlighted row is the one whose first-letter style differs from the others
//! (`assert_only_highlighted`). Theme items are searched in the 80x24 items column (x 20..40),
//! Monitors items in the 120x24 items column (x 21..53), module labels in x 0..20. The status row
//! text is searched on every rendered row (its location is not fixed). Only `System` is faked.

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::*;
use ln_hub::{App, Config, ConfigError};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;

const T_X_START: u16 = 20;
const T_X_END: u16 = 40;
const M_X_START: u16 = 21;
const M_X_END: u16 = 53;
const MODULE_X_END: u16 = 20;
const WIDE_W: u16 = 120;
const WIDE_H: u16 = 24;

/// Sorted order a1, b1, b2, c1.
const ABC: [&str; 4] = ["a1.png", "b1.png", "b2.png", "c1.png"];

const EDP: &str = "eDP-1  2880x1800@120  on";
const DP: &str = "DP-1  2560x1440@60  on";
const HDMI: &str = "HDMI-A-1  1920x1080@60  on";
const MONITOR_ROWS: [&str; 3] = [EDP, DP, HDMI];
const MONITORS_JSON: &str = r#"[
  {"name": "eDP-1", "width": 2880, "height": 1800, "refreshRate": 120.000, "disabled": false},
  {"name": "DP-1", "width": 2560, "height": 1440, "refreshRate": 60.000, "disabled": false},
  {"name": "HDMI-A-1", "width": 1920, "height": 1080, "refreshRate": 60.000, "disabled": false}
]"#;

/// App over a real temp wallpaper dir holding `files`, Theme items focused (`l`).
fn focused(name: &str, files: &[&str]) -> (App, FakeSystem, PathBuf) {
    let home = tempdir(&format!("{name}-home"));
    let dir = tempdir(&format!("{name}-wall"));
    for f in files {
        fs::write(dir.join(f), b"x").unwrap();
    }
    let mut cfg = Config::default_for(&home);
    cfg.wallpaper_dir = dir.clone();
    let sys = FakeSystem::new();
    let mut app = App::new(cfg, Box::new(sys.clone()));
    press(&mut app, 'l');
    (app, sys, dir)
}

fn type_str(app: &mut App, s: &str) {
    for c in s.chars() {
        press(app, c);
    }
}

fn theme_hit(buf: &Buffer, text: &str) -> Option<(u16, u16)> {
    items_hits(buf, text, T_X_START, T_X_END).first().copied()
}

fn assert_theme_hl(app: &mut App, rows: &[&str], expected: &str) {
    let buf = draw(app);
    assert_only_highlighted(&buf, rows, expected, theme_hit);
}

fn rendered<'a>(buf: &Buffer, names: &[&'a str]) -> Vec<&'a str> {
    let mut found: Vec<(u16, &str)> = names
        .iter()
        .filter_map(|n| theme_hit(buf, n).map(|(_, y)| (y, *n)))
        .collect();
    found.sort_by_key(|&(y, _)| y);
    found.into_iter().map(|(_, n)| n).collect()
}

fn spawn_for(dir: &Path, file: &str) -> (String, Vec<String>) {
    (
        "ln-themectl".to_string(),
        vec!["-w".to_string(), format!("{}/{file}", dir.display())],
    )
}

fn wide_shows(app: &mut App, text: &str) -> bool {
    buffer_lines(&draw_sized(app, WIDE_W, WIDE_H))
        .iter()
        .any(|l| l.contains(text))
}

fn assert_wide_shows(app: &mut App, text: &str) {
    assert_status(&draw_sized(app, WIDE_W, WIDE_H), text);
}

fn monitors_sys() -> FakeSystem {
    let sys = FakeSystem::new();
    sys.set_stdout("hyprctl", MONITORS_JSON);
    sys
}

fn assert_monitor_hl(app: &mut App, expected: &str) {
    let buf = draw_sized(app, WIDE_W, WIDE_H);
    assert_only_highlighted(&buf, &MONITOR_ROWS, expected, |b, t| {
        items_hit_within(b, t, M_X_START, M_X_END)
    });
}

/// Style of the module label `name` in the module column.
fn module_style(app: &mut App, name: &str) -> CellStyle {
    name_style(&draw(app), name, 0, MODULE_X_END)
}

/// Asserts `expected` is the highlighted module: with only two modules the other label keeps the
/// unselected style `base` (captured before any key) and the expected one differs from it.
fn assert_module_hl(app: &mut App, base: CellStyle, expected: &str) {
    let other = if expected == "Theme" {
        "Monitors"
    } else {
        "Theme"
    };
    assert_eq!(
        module_style(app, other),
        base,
        "`{other}` must be unselected"
    );
    assert_ne!(
        module_style(app, expected),
        base,
        "`{expected}` must be the selected module"
    );
}

// ---- G / gg ----

#[test]
fn g_and_gg_jump_to_the_last_and_first_theme_item() {
    // Catches: `G` / `gg` unbound in the Theme items (cursor stays on row 0 after `G`), or `gg`
    // not returning to the first row.
    let (mut app, _sys, _dir) = focused("s13k-theme", &ABC);
    assert_theme_hl(&mut app, &ABC, "a1.png");
    press(&mut app, 'G');
    assert_theme_hl(&mut app, &ABC, "c1.png");
    press(&mut app, 'g');
    press(&mut app, 'g');
    assert_theme_hl(&mut app, &ABC, "a1.png");
}

#[test]
fn g_and_gg_jump_between_modules_in_the_module_column() {
    // Catches: `G` / `gg` ignored while the module column is focused (selection stays on Theme),
    // or the new selection not driving the items shown after `l`.
    let sys = FakeSystem::new();
    sys.set_stdout("hyprctl", MONITORS_JSON);
    let mut app = app_after_keys("s13k-modules", &sys, &[]);
    let base = module_style(&mut app, "Monitors"); // unselected style (Theme starts selected)
    assert_module_hl(&mut app, base, "Theme");
    press(&mut app, 'G');
    assert_module_hl(&mut app, base, "Monitors");
    press(&mut app, 'l');
    assert_status(&draw_sized(&mut app, WIDE_W, WIDE_H), EDP);
    press(&mut app, 'h');
    assert_module_hl(&mut app, base, "Monitors");
    press(&mut app, 'g');
    press(&mut app, 'g');
    assert_module_hl(&mut app, base, "Theme");
    press(&mut app, 'l');
    assert_status(&draw_sized(&mut app, WIDE_W, WIDE_H), "img1.png");
}

#[test]
fn g_and_gg_jump_to_the_last_and_first_monitor() {
    // Catches: `G` / `gg` not wired in the Monitors pane (it has its own cursor handling).
    let sys = monitors_sys();
    let mut app = monitors_app("s13k-monitors", &sys);
    assert_monitor_hl(&mut app, EDP);
    press(&mut app, 'G');
    assert_monitor_hl(&mut app, HDMI);
    press(&mut app, 'g');
    press(&mut app, 'g');
    assert_monitor_hl(&mut app, EDP);
}

#[test]
fn g_and_gg_use_the_visible_rows_of_an_active_filter() {
    // Catches: `G` / `gg` indexing the unfiltered list. Filter `b` leaves b1, b2; unfiltered
    // `G` would land on c1 (and apply c1.png), unfiltered `gg` on a1.
    let (mut app, sys, dir) = focused("s13k-filter-g", &ABC);
    press(&mut app, '/');
    press(&mut app, 'b');
    enter(&mut app);
    assert_eq!(rendered(&draw(&mut app), &ABC), vec!["b1.png", "b2.png"]);
    press(&mut app, 'G');
    assert_theme_hl(&mut app, &["b1.png", "b2.png"], "b2.png");
    enter(&mut app);
    assert_eq!(sys.spawn_calls(), vec![spawn_for(&dir, "b2.png")]);

    let (mut app, sys, dir) = focused("s13k-filter-gg", &ABC);
    press(&mut app, '/');
    press(&mut app, 'b');
    enter(&mut app);
    press(&mut app, 'G');
    assert_theme_hl(&mut app, &["b1.png", "b2.png"], "b2.png");
    press(&mut app, 'g');
    press(&mut app, 'g');
    assert_theme_hl(&mut app, &["b1.png", "b2.png"], "b1.png");
    enter(&mut app);
    assert_eq!(sys.spawn_calls(), vec![spawn_for(&dir, "b1.png")]);
}

// ---- lone g swallows the next key ----

#[test]
fn g_then_another_key_swallows_that_key() {
    // Catches: `g` being inert so the following `j` still moves (cursor on row 1), i.e. no
    // pending-`g` state.
    let (mut app, _sys, _dir) = focused("s13k-swallow-j", &ABC);
    press(&mut app, 'g');
    press(&mut app, 'j');
    assert_theme_hl(&mut app, &ABC, "a1.png");
    press(&mut app, 'j'); // the pending `g` is gone: this one moves
    assert_theme_hl(&mut app, &ABC, "b1.png");
}

#[test]
fn g_then_quit_key_does_not_quit_and_the_next_quit_key_does() {
    // Catches: the quit key still acting after a lone `g`, or the swallow lasting longer than
    // one key.
    let (mut app, _sys, _dir) = focused("s13k-swallow-q", &ABC);
    press(&mut app, 'g');
    press(&mut app, 'q');
    assert!(!app.should_quit(), "`g` then `q`: the `q` is swallowed");
    press(&mut app, 'q');
    assert!(app.should_quit(), "a second `q` quits normally");
}

#[test]
fn g_then_enter_does_not_apply() {
    // Catches: a non-character second key (Enter) bypassing the pending-`g` swallow.
    let (mut app, sys, _dir) = focused("s13k-swallow-enter", &ABC);
    press(&mut app, 'g');
    enter(&mut app);
    assert!(
        sys.spawn_calls().is_empty(),
        "`g` then Enter must not apply, got {:?}",
        sys.spawn_calls()
    );
    enter(&mut app);
    assert_eq!(sys.spawn_calls().len(), 1, "a later Enter applies normally");
}

#[test]
fn g_then_capital_g_swallows_the_second_key_and_moves_nothing() {
    // Catches: `gG` treated as `G` (cursor jumping to the last row).
    let (mut app, _sys, _dir) = focused("s13k-swallow-G", &ABC);
    press(&mut app, 'g');
    press(&mut app, 'G');
    assert_theme_hl(&mut app, &ABC, "a1.png");
}

// ---- prompts: g / G / ? are text ----

#[test]
fn g_capital_g_and_question_mark_are_text_in_the_filter_prompt() {
    // Catches: `g` / `gg` / `G` / `?` handled as commands while the filter prompt is open.
    let files = ["alpha.png", "ggx.png", "Gong.png", "song.png", "zeta.png"];
    let (mut app, _sys, _dir) = focused("s13k-filter-text", &files);
    press(&mut app, '/');
    press(&mut app, 'G');
    assert_app_status(&mut app, "Filter: G");
    type_str(&mut app, "on");
    assert_app_status(&mut app, "Filter: Gon");
    assert_eq!(rendered(&draw(&mut app), &files), vec!["Gong.png"]);

    let (mut app, _sys, _dir) = focused("s13k-filter-gg-text", &files);
    press(&mut app, '/');
    type_str(&mut app, "gg");
    assert_app_status(&mut app, "Filter: gg");
    assert_eq!(rendered(&draw(&mut app), &files), vec!["ggx.png"]);

    let (mut app, _sys, _dir) = focused("s13k-filter-q-text", &files);
    press(&mut app, '/');
    press(&mut app, '?');
    assert_app_status(&mut app, "Filter: ?");
    assert!(
        !wide_shows(&mut app, "top/bottom"),
        "`?` typed in the prompt must not show the help"
    );
}

#[test]
fn g_capital_g_and_question_mark_are_text_in_the_font_name_prompt() {
    // Catches: `g` / `G` / `?` handled as commands (or dropped) while the font-name prompt is
    // open: they must be echoed and submitted as part of the name.
    let (mut app, sys, _dir) = focused("s13k-font-text", &ABC);
    press(&mut app, 'f');
    type_str(&mut app, "g?G");
    let lines = buffer_lines(&draw(&mut app));
    assert!(
        lines
            .iter()
            .any(|l| l.contains("Font name:") && l.contains("g?G")),
        "prompt line must show `Font name:` and `g?G`; buffer:\n{}",
        lines.join("\n")
    );
    assert!(!app.should_quit());
    enter(&mut app);
    assert_eq!(
        sys.spawn_calls(),
        vec![(
            "ln-themectl".to_string(),
            vec!["-f".to_string(), "g?G".to_string()]
        )]
    );
}

// ---- ? help ----

#[test]
fn question_mark_shows_the_help_with_the_quit_key_and_moves_nothing() {
    // Catches: `?` unbound (no help), help missing the `gg/G` hint or the quit key, or `?`
    // moving the cursor / changing focus.
    let (mut app, _sys, _dir) = focused("s13k-help", &ABC);
    press(&mut app, 'j');
    assert_theme_hl(&mut app, &ABC, "b1.png");
    press(&mut app, '?');
    assert_wide_shows(&mut app, "gg/G top/bottom");
    assert_wide_shows(&mut app, "q quit");
    assert_theme_hl(&mut app, &ABC, "b1.png");
    assert!(!app.should_quit());
    press(&mut app, 'j'); // still in the items column, no prompt open
    assert_theme_hl(&mut app, &ABC, "b2.png");
}

#[test]
fn question_mark_shows_the_help_in_the_module_column_too() {
    // Catches: `?` handled only inside a pane, so it does nothing while the modules are focused.
    let sys = FakeSystem::new();
    let mut app = app_after_keys("s13k-help-mod", &sys, &[]);
    let base = module_style(&mut app, "Monitors");
    press(&mut app, '?');
    assert_wide_shows(&mut app, "gg/G top/bottom");
    assert_module_hl(&mut app, base, "Theme");
}

#[test]
fn help_names_the_configured_quit_key() {
    // Catches: a hard-coded `q quit` in the help text instead of the configured key.
    let cfg = Config::from_toml("[keymap]\nquit = \"x\"", Path::new("/h")).unwrap();
    let mut app = App::new(cfg, Box::new(FakeSystem::new()));
    press(&mut app, '?');
    assert_wide_shows(&mut app, "x quit");
    assert!(
        !wide_shows(&mut app, "q quit"),
        "the default key is not bound"
    );
    assert_wide_shows(&mut app, "gg/G top/bottom");
}

// ---- config ----

#[test]
fn quit_key_may_not_be_g_capital_g_or_question_mark() {
    // Catches: `g`, `G` or `?` accepted as the quit key, shadowing the new bindings.
    for k in ['g', 'G', '?'] {
        match Config::from_toml(&format!("[keymap]\nquit = \"{k}\""), Path::new("/h")) {
            Err(ConfigError::KeyConflict { action, key }) => {
                assert_eq!(action, "quit");
                assert_eq!(key, k);
            }
            other => panic!("quit = {k:?}: expected Err(KeyConflict), got {other:?}"),
        }
    }
}

#[test]
fn esc_after_g_is_swallowed_and_the_next_g_starts_a_fresh_pending_state() {
    // Catches: Esc (a non-char key) not consuming the pending `g`, so the next `g` completes `gg`.
    // Flow: g, Esc (swallowed second key), then a single `g` + `j`: `j` must be swallowed again
    // (a fresh pending `g`), leaving the cursor on the row it was on.
    let (mut app, _sys, _dir) = focused("s13k-esc", &ABC);
    press(&mut app, 'j');
    press(&mut app, 'g');
    app.handle_key(key(KeyCode::Esc));
    press(&mut app, 'g');
    press(&mut app, 'j');
    assert_theme_hl(&mut app, &ABC, "b1.png");
}

//! S17-help-status-overlap: the right-aligned `Font: ...` summary never overprints the left text of
//! the status row (help line, status message, prompt, startup text). The left text always wins; the
//! font summary is drawn only when `width(left) + 1 + width(F) <= width(row)`, otherwise it is
//! omitted whole (never truncated).
//!
//! Setup: Theme module, Items column focused, `env.lua` with `JetBrains Mono` size 12, default quit
//! key `q`. The status row is the LAST row of the render. Every expected row is built from literal
//! strings and explicit blank counts, never from App output. Only `System` is faked.

mod common;

use std::fs;

use common::*;
use ln_hub::{App, Config, JobState};

/// The help line with the default quit key: 80 cells (S13).
const H: &str = "h/j/k/l move  gg/G top/bottom  ⏎ apply  / filter  f font  s size  ? help  q quit";
/// The first 70 cells of `H` (hand-cut: `H` up to and including `? he`).
const H70: &str = "h/j/k/l move  gg/G top/bottom  ⏎ apply  / filter  f font  s size  ? he";
/// The font summary for the fixture: 29 cells.
const F: &str = "Font: JetBrains Mono, size 12";
/// `Apply failed: ` (14 cells) plus a 56-cell reason: 70 cells.
const FAILED_70: &str = "Apply failed: ln-themectl exited with status 1: image file unreadable.";
/// `Apply failed: ` plus a 70-cell reason: 84 cells, wider than an 80-column row.
const FAILED_84: &str =
    "Apply failed: ln-themectl exited with status 1: image file unreadable. (see the log)";
const ENV_LUA: &str = "hl.env(\"FONT_NAME\", \"JetBrains Mono\")\nhl.env(\"FONT_SIZE\", \"12\")\n";

fn spaces(n: usize) -> String {
    " ".repeat(n)
}

/// Theme module, Items focused, a wallpaper `wp.png`, the JetBrains Mono / 12 env file.
fn theme_app(name: &str, sys: &FakeSystem) -> App {
    let home = tempdir(&format!("{name}-home"));
    let dir = tempdir(&format!("{name}-wall"));
    fs::write(dir.join("wp.png"), b"x").unwrap();
    let env_file = tempdir(&format!("{name}-hypr")).join("env.lua");
    fs::write(&env_file, ENV_LUA).unwrap();
    let mut cfg = Config::default_for(&home);
    cfg.wallpaper_dir = dir;
    cfg.hypr_env_file = env_file;
    let mut app = App::new(cfg, Box::new(sys.clone()));
    press(&mut app, 'l');
    app
}

/// The status row (last row) of a render at `w` x 24, cell by cell, with the buffer for messages.
fn status_row(app: &mut App, w: u16) -> (String, String) {
    let buf = draw_sized(app, w, DRAW_H);
    let row = buffer_lines(&buf).last().unwrap().clone();
    (row, dump(&buf))
}

/// Asserts the status row at width `w` is exactly `expected`, blanks included (so the row is
/// `expected.len() == w` cells), and so holds no stray part of `F` or of the left text.
fn assert_row_is(app: &mut App, w: u16, expected: &str) {
    let (row, dump) = status_row(app, w);
    assert_eq!(row, expected, "status row at {w} columns; buffer:\n{dump}");
}

/// Asserts the row at `w` is `left` clipped/padded by blanks to the width, with no `Font:` in it.
fn assert_left_only(app: &mut App, w: u16, left: &str) {
    let padded = format!(
        "{left}{}",
        spaces((w as usize).saturating_sub(left.chars().count()))
    );
    assert_row_is(app, w, &padded);
}

#[test]
fn help_at_80_columns_is_whole_and_font_is_hidden() {
    // Catches: `Font: ...` taking its 29 columns and cutting `H` at column 51 (`... f font` +
    // `Font: ...`); the help line must stay whole and F must be omitted (80 < 80 + 1 + 29).
    let sys = FakeSystem::new();
    let mut app = theme_app("s17-help-80", &sys);
    press(&mut app, '?');
    assert_left_only(&mut app, 80, H);
    let (row, dump) = status_row(&mut app, 80);
    assert!(
        row.ends_with("q quit"),
        "row `{row}` must end with `q quit`; buffer:\n{dump}"
    );
    assert!(
        !row.contains("Font:"),
        "no `Font:` in the row `{row}`; buffer:\n{dump}"
    );
}

#[test]
fn help_at_70_columns_is_clipped_by_the_row_edge_only_and_font_is_hidden() {
    // Catches: F overprinting the right of the 70-cell help (the left text cut at 41 and `Font:`
    // after it); the row must be exactly the first 70 cells of `H`.
    let sys = FakeSystem::new();
    let mut app = theme_app("s17-help-70", &sys);
    press(&mut app, '?');
    assert_row_is(&mut app, 70, H70);
}

#[test]
fn help_at_120_columns_is_whole_and_font_is_whole_and_right_aligned() {
    // Catches: over-eager hiding (F dropped although 80 + 1 + 29 <= 120), F not right-aligned, or
    // F touching `H`. 120 - 80 - 29 = 11 blank cells between them.
    let sys = FakeSystem::new();
    let mut app = theme_app("s17-help-120", &sys);
    press(&mut app, '?');
    assert_row_is(&mut app, 120, &format!("{H}{}{F}", spaces(11)));
}

#[test]
fn at_exactly_len_h_plus_1_plus_len_f_both_show_with_one_blank_between() {
    // Catches: an off-by-one that hides F when the room is exactly enough (80 + 1 + 29 = 110).
    let sys = FakeSystem::new();
    let mut app = theme_app("s17-boundary-110", &sys);
    press(&mut app, '?');
    assert_row_is(&mut app, 110, &format!("{H} {F}"));
}

#[test]
fn one_cell_below_the_boundary_hides_font_and_keeps_help_whole() {
    // Catches: F drawn flush against `H` with no blank (the old layout at 109 = 80 + 29), or F
    // truncated/partly drawn; at 109 the room for a blank is missing so F is omitted whole.
    let sys = FakeSystem::new();
    let mut app = theme_app("s17-boundary-109", &sys);
    press(&mut app, '?');
    assert_left_only(&mut app, 109, H);
}

#[test]
fn below_the_boundary_font_is_never_shown_partly() {
    // Catches: a shrunk or clipped F (`Font: JetBrains`, `Font:`) appearing at a width between
    // `len(H)` and `len(H) + 1 + len(F)`.
    let sys = FakeSystem::new();
    let mut app = theme_app("s17-boundary-sweep", &sys);
    press(&mut app, '?');
    for w in [81u16, 90, 100, 108] {
        assert_left_only(&mut app, w, H);
    }
}

#[test]
fn no_status_at_80_columns_shows_font_right_aligned() {
    // Catches: F hidden when there is no left text (regression); 80 - 29 = 51 blank cells.
    let sys = FakeSystem::new();
    let mut app = theme_app("s17-nostatus-80", &sys);
    assert_row_is(&mut app, 80, &format!("{}{F}", spaces(51)));
}

#[test]
fn short_status_keeps_font_at_80_columns() {
    // Catches: F hidden whenever any status exists; `Applying wp.png…` (16 cells) plus F fits in
    // 80 columns, so both show whole: 80 - 16 - 29 = 35 blank cells between them.
    let sys = FakeSystem::new();
    let mut app = theme_app("s17-short-status", &sys);
    enter(&mut app);
    assert_row_is(&mut app, 80, &format!("Applying wp.png…{}{F}", spaces(35)));
}

#[test]
fn font_comes_back_once_the_long_text_is_replaced_by_a_short_one() {
    // Catches: a hide decision remembered across frames (state) instead of decided per frame:
    // help hides F, then `Applying wp.png…` replaces it and F must be back.
    let sys = FakeSystem::new();
    let mut app = theme_app("s17-per-frame", &sys);
    press(&mut app, '?');
    assert_left_only(&mut app, 80, H);
    enter(&mut app);
    assert_row_is(&mut app, 80, &format!("Applying wp.png…{}{F}", spaces(35)));
}

#[test]
fn long_status_at_80_columns_is_whole_and_hides_font() {
    // Catches: the end of `Apply failed: ...` replaced by F (the left text cut at column 51);
    // 70 + 1 + 29 = 100 > 80, so F is omitted and the 70-cell status is whole.
    let sys = FakeSystem::new();
    sys.set_job_state(JobState::Failed(
        "ln-themectl exited with status 1: image file unreadable.".to_string(),
    ));
    let mut app = theme_app("s17-long-status", &sys);
    enter(&mut app);
    app.tick();
    assert_left_only(&mut app, 80, FAILED_70);
}

#[test]
fn status_wider_than_the_row_is_clipped_by_the_row_edge_and_hides_font() {
    // Catches: F drawn over the end of a status already wider than the row (84 cells at 80):
    // the left text is clipped by the row edge only, and no part of F appears.
    let sys = FakeSystem::new();
    sys.set_job_state(JobState::Failed(
        "ln-themectl exited with status 1: image file unreadable. (see the log)".to_string(),
    ));
    let mut app = theme_app("s17-wide-status", &sys);
    enter(&mut app);
    app.tick();
    let first_80: String = FAILED_84.chars().take(80).collect();
    assert_row_is(&mut app, 80, &first_80);
}

#[test]
fn a_prompt_wider_than_the_room_is_whole_and_hides_font() {
    // Catches: the prompt cut at column 51 by F. `Font name: ` (11 cells) + 40 typed = 51 cells;
    // 51 + 1 + 29 = 81 > 80, so F is omitted and the prompt is whole.
    let sys = FakeSystem::new();
    let mut app = theme_app("s17-prompt-wide", &sys);
    press(&mut app, 'f');
    for _ in 0..40 {
        press(&mut app, 'x');
    }
    assert_left_only(&mut app, 80, &format!("Font name: {}", "x".repeat(40)));
}

#[test]
fn a_prompt_that_just_fits_next_to_font_keeps_it_with_one_blank() {
    // Catches: hiding F one cell too early for a prompt: 11 + 39 = 50 cells, 50 + 1 + 29 = 80 <= 80,
    // so both show with exactly one blank between them.
    let sys = FakeSystem::new();
    let mut app = theme_app("s17-prompt-fit", &sys);
    press(&mut app, 'f');
    for _ in 0..39 {
        press(&mut app, 'x');
    }
    assert_row_is(&mut app, 80, &format!("Font name: {} {F}", "x".repeat(39)));
}

#[test]
fn a_short_prompt_keeps_font() {
    // Catches: F hidden whenever a prompt is open; `Font name: ab` is 13 cells, 80 - 13 - 29 = 38
    // blank cells before F.
    let sys = FakeSystem::new();
    let mut app = theme_app("s17-prompt-short", &sys);
    press(&mut app, 'f');
    press(&mut app, 'a');
    press(&mut app, 'b');
    assert_row_is(&mut app, 80, &format!("Font name: ab{}{F}", spaces(38)));
}

#[test]
fn startup_text_keeps_font_when_it_fits() {
    // Catches: the rule not applying uniformly, or hiding F for the short startup text:
    // `Initializing…` (13 cells) + F fits at 80 columns, 80 - 13 - 29 = 38 blank cells.
    let sys = FakeSystem::new();
    let mut app = theme_app("s17-startup", &sys);
    app.init_started();
    assert_row_is(&mut app, 80, &format!("Initializing…{}{F}", spaces(38)));
}

#[test]
fn module_column_focus_leaves_the_help_line_unchanged_at_80_and_70_columns() {
    // Catches: a change to the module-column view (F is not shown there): help whole at 80, the
    // first 70 cells at 70, no `Font:`.
    let sys = FakeSystem::new();
    let mut app = app_after_keys("s17-module-focus", &sys, &['?']);
    assert_left_only(&mut app, 80, H);
    assert_row_is(&mut app, 70, H70);
}

#[test]
fn empty_left_at_exactly_font_width_draws_font_with_no_blank() {
    // Catches: the fit rule applied with `+ 1` even when there is no left text, hiding F on a
    // 29-column row (0 + 1 + 29 > 29); with no left text no separating blank is needed.
    let sys = FakeSystem::new();
    let mut app = theme_app("s17-empty-29", &sys);
    assert_row_is(&mut app, 29, F);
}

#[test]
fn empty_left_narrower_than_font_draws_font_clipped_by_the_row_edge_only() {
    // Catches: F hidden whole on a row narrower than F when there is no left text; the spec says F
    // is drawn as today there, clipped by the row. The first 20 cells of F are `Font: JetBrains Mono`
    // (`Font:` 5 + blank 1 + `JetBrains` 9 + blank 1 + `Mono` 4 = 20).
    let sys = FakeSystem::new();
    let mut app = theme_app("s17-empty-20", &sys);
    assert_row_is(&mut app, 20, "Font: JetBrains Mono");
}

#[test]
fn empty_left_one_cell_wider_than_font_is_right_aligned_with_one_blank() {
    // Catches: a regression in right alignment for empty left text at 30 columns (30 - 29 = 1 blank
    // before F).
    let sys = FakeSystem::new();
    let mut app = theme_app("s17-empty-30", &sys);
    assert_row_is(&mut app, 30, &format!(" {F}"));
}

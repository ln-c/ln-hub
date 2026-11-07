//! S12-filter: `/` opens a case-insensitive substring filter over the Theme wallpaper rows.
//!
//! Observables (80x24 `TestBackend`): a wallpaper row is "rendered" when its name starts inside
//! the items column (x in 20..40); the preview is the column from x = 40; the status row text is
//! searched on every rendered row (its location is not fixed). The highlighted row is the one
//! whose first-letter style differs from the others (`assert_only_highlighted`).
//!
//! Wallpaper names sort naturally and case-insensitively, so the row order is the sorted order.

mod common;

use std::fs;
use std::path::PathBuf;

use common::*;
use ln_hub::{App, Config};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;

const ITEMS_X_START: u16 = 20;
const ITEMS_X_END: u16 = 40;
const PREVIEW_X_START: u16 = 40;

/// The spec's acceptance set (sorted order: cat.mp4, frieren.png, img2.png).
const SPEC: [&str; 3] = ["cat.mp4", "frieren.png", "img2.png"];
/// Index-desync set (sorted order): a1, b1, b2, c1. Filter `b` leaves b1 (unfiltered index 1)
/// and b2 (unfiltered index 2) at filtered indices 0 and 1.
const ABC: [&str; 4] = ["a1.png", "b1.png", "b2.png", "c1.png"];

/// App over a real temp wallpaper dir holding `files`; `current` (a file name) is written to the
/// current-wallpaper file when given. Returns the app, its fake system and the wallpaper dir.
fn setup(name: &str, files: &[&str], current: Option<&str>) -> (App, FakeSystem, PathBuf) {
    let home = tempdir(&format!("{name}-home"));
    let dir = tempdir(&format!("{name}-wall"));
    for f in files {
        fs::write(dir.join(f), b"x").unwrap();
    }
    let mut cfg = Config::default_for(&home);
    cfg.wallpaper_dir = dir.clone();
    if let Some(cur) = current {
        let file = tempdir(&format!("{name}-cache")).join("current_wallpaper");
        fs::write(&file, format!("{}/{cur}\n", dir.display())).unwrap();
        cfg.current_wallpaper_file = file;
    }
    let sys = FakeSystem::new();
    let app = App::new(cfg, Box::new(sys.clone()));
    (app, sys, dir)
}

/// Theme items focused (`l`) over `files`, nothing pressed after that.
fn focused(name: &str, files: &[&str]) -> (App, FakeSystem, PathBuf) {
    let (mut app, sys, dir) = setup(name, files, None);
    press(&mut app, 'l');
    (app, sys, dir)
}

fn type_str(app: &mut App, s: &str) {
    for c in s.chars() {
        press(app, c);
    }
}

fn esc(app: &mut App) {
    app.handle_key(key(KeyCode::Esc));
}

fn backspace(app: &mut App) {
    app.handle_key(key(KeyCode::Backspace));
}

/// Which of `names` are rendered in the items column, in top-to-bottom order.
fn rendered<'a>(buf: &Buffer, names: &[&'a str]) -> Vec<&'a str> {
    let mut found: Vec<(u16, &str)> = names
        .iter()
        .filter_map(|n| {
            items_hits(buf, n, ITEMS_X_START, ITEMS_X_END)
                .first()
                .map(|&(_, y)| (y, *n))
        })
        .collect();
    found.sort_by_key(|&(y, _)| y);
    found.into_iter().map(|(_, n)| n).collect()
}

fn assert_rendered(app: &mut App, names: &[&str], expected: &[&str]) {
    let buf = draw(app);
    assert_eq!(
        rendered(&buf, names),
        expected,
        "rendered rows; buffer:\n{}",
        dump(&buf)
    );
}

/// Asserts `expected` is the only highlighted row among the currently rendered `rows`.
fn assert_highlighted(app: &mut App, rows: &[&str], expected: &str) {
    let buf = draw(app);
    assert_only_highlighted(&buf, rows, expected, |b, n| {
        items_hits(b, n, ITEMS_X_START, ITEMS_X_END)
            .first()
            .copied()
    });
}

fn shows(app: &mut App, text: &str) -> bool {
    buffer_lines(&draw(app)).iter().any(|l| l.contains(text))
}

fn assert_shows(app: &mut App, text: &str) {
    assert_app_status(app, text);
}

fn assert_lacks(app: &mut App, text: &str) {
    let buf = draw(app);
    assert!(
        !buffer_lines(&buf).iter().any(|l| l.contains(text)),
        "`{text}` must not be shown; buffer:\n{}",
        dump(&buf)
    );
}

fn spawn_for(dir: &std::path::Path, file: &str) -> (String, Vec<String>) {
    (
        "ln-themectl".to_string(),
        vec!["-w".to_string(), format!("{}/{file}", dir.display())],
    )
}

// ---- acceptance ----

#[test]
fn typing_filters_the_rows_and_esc_restores_them() {
    // Catches: `/` doing nothing, rows not narrowing to the typed text, or Esc not restoring them.
    let (mut app, _sys, _dir) = focused("s12-accept", &SPEC);
    assert_rendered(&mut app, &SPEC, &SPEC);
    press(&mut app, '/');
    type_str(&mut app, "fri");
    assert_rendered(&mut app, &SPEC, &["frieren.png"]);
    esc(&mut app);
    assert_rendered(&mut app, &SPEC, &SPEC);
}

#[test]
fn filter_is_case_insensitive_for_the_typed_text() {
    // Catches: a case-sensitive comparison (`FRI` finding nothing).
    let (mut app, _sys, _dir) = focused("s12-case-typed", &SPEC);
    press(&mut app, '/');
    type_str(&mut app, "FRI");
    assert_rendered(&mut app, &SPEC, &["frieren.png"]);
}

#[test]
fn filter_is_case_insensitive_for_the_wallpaper_name() {
    // Catches: lowercasing only the typed text, so an upper-case name (`Zeta.png`) never matches.
    let names = ["other.png", "Zeta.png"];
    let (mut app, _sys, _dir) = focused("s12-case-name", &names);
    press(&mut app, '/');
    type_str(&mut app, "zet");
    assert_rendered(&mut app, &names, &["Zeta.png"]);
}

#[test]
fn filter_matches_a_substring_not_only_a_prefix() {
    // Catches: starts_with matching instead of substring matching.
    let (mut app, _sys, _dir) = focused("s12-substr", &SPEC);
    press(&mut app, '/');
    type_str(&mut app, "eren");
    assert_rendered(&mut app, &SPEC, &["frieren.png"]);
}

#[test]
fn filter_matches_the_name_not_the_directory_path() {
    // Catches: matching the full path (every temp path contains `ln-hub`) so nothing is filtered.
    let (mut app, _sys, _dir) = focused("s12-path", &SPEC);
    press(&mut app, '/');
    type_str(&mut app, "ln-hub");
    assert_shows(&mut app, "Filter: ln-hub");
    assert_rendered(&mut app, &SPEC, &[]);
}

// ---- where `/` works ----

#[test]
fn slash_in_the_module_column_does_nothing() {
    // CHARACTERISATION GUARD (passes before S12): `/` must not open the filter from the module
    // column, so the following `l` still focuses the items and `q` still quits.
    let (mut app, _sys, _dir) = setup("s12-slash-modules", &SPEC, None);
    press(&mut app, '/');
    assert_lacks(&mut app, "Filter:");
    press(&mut app, 'l');
    assert_highlighted(&mut app, &SPEC, "cat.mp4");
    assert!(!app.should_quit());
    let (mut app, _sys, _dir) = setup("s12-slash-modules-q", &SPEC, None);
    press(&mut app, '/');
    press(&mut app, 'q');
    assert!(app.should_quit(), "`q` must still quit after a stray `/`");
}

#[test]
fn slash_in_the_monitors_items_does_nothing() {
    // CHARACTERISATION GUARD (passes before S12): the filter is Theme-only in v1, so `/` in the
    // Monitors items neither opens a prompt nor swallows `q`.
    let sys = FakeSystem::new();
    let mut app = monitors_app("s12-slash-monitors", &sys);
    press(&mut app, '/');
    assert_lacks(&mut app, "Filter:");
    press(&mut app, 'q');
    assert!(app.should_quit(), "`q` must still quit after a stray `/`");
}

// ---- the open input ----

#[test]
fn open_input_shows_the_typed_text_in_the_status_row() {
    // Catches: no `Filter: <text>` status while the input is open (or text not tracked).
    let (mut app, _sys, _dir) = focused("s12-status", &SPEC);
    press(&mut app, '/');
    assert_shows(&mut app, "Filter: ");
    type_str(&mut app, "fri");
    assert_shows(&mut app, "Filter: fri");
}

#[test]
fn every_printable_key_is_text_while_the_input_is_open() {
    // Catches: `q`, `j`, `k`, `h`, `l`, `f`, `s` still acting as keys (quit, move, unfocus,
    // refocus) instead of filter text while typing.
    let names = ["hjklqfs.png", "other.png", "plain.png"];
    let (mut app, _sys, _dir) = focused("s12-keys", &names);
    press(&mut app, '/');
    type_str(&mut app, "hjklqfs");
    assert!(
        !app.should_quit(),
        "`q` must be text while the input is open"
    );
    assert_shows(&mut app, "Filter: hjklqfs");
    assert_rendered(&mut app, &names, &["hjklqfs.png"]);
    assert_highlighted(&mut app, &["hjklqfs.png"], "hjklqfs.png");
}

#[test]
fn backspace_deletes_the_last_char_and_widens_the_rows() {
    // Catches: Backspace ignored, clearing everything, or rows not recomputed after deleting.
    let (mut app, _sys, _dir) = focused("s12-backspace", &SPEC);
    press(&mut app, '/');
    type_str(&mut app, "frx");
    assert_rendered(&mut app, &SPEC, &[]);
    backspace(&mut app);
    assert_shows(&mut app, "Filter: fr");
    assert_lacks(&mut app, "Filter: frx");
    assert_rendered(&mut app, &SPEC, &["frieren.png"]);
    backspace(&mut app);
    backspace(&mut app);
    assert_rendered(&mut app, &SPEC, &SPEC);
}

#[test]
fn cursor_goes_to_the_first_match_whenever_the_text_changes() {
    // Catches: the cursor staying on its old index (or old item) when the filter narrows or
    // widens; also covers Backspace changes.
    let (mut app, _sys, _dir) = focused("s12-cursor", &ABC);
    for _ in 0..3 {
        press(&mut app, 'j'); // cursor on c1.png (unfiltered index 3)
    }
    assert_highlighted(&mut app, &ABC, "c1.png");
    press(&mut app, '/');
    press(&mut app, 'b');
    assert_rendered(&mut app, &ABC, &["b1.png", "b2.png"]);
    assert_highlighted(&mut app, &["b1.png", "b2.png"], "b1.png");
    press(&mut app, '2');
    assert_rendered(&mut app, &ABC, &["b2.png"]);
    assert_highlighted(&mut app, &["b2.png"], "b2.png");
    backspace(&mut app);
    assert_rendered(&mut app, &ABC, &["b1.png", "b2.png"]);
    assert_highlighted(&mut app, &["b1.png", "b2.png"], "b1.png");
}

// ---- closing with Enter and applying ----

#[test]
fn enter_closes_the_input_but_keeps_the_filter() {
    // Catches: Enter discarding the filter, or leaving the `Filter:` prompt open.
    let (mut app, _sys, _dir) = focused("s12-enter-keeps", &SPEC);
    press(&mut app, '/');
    type_str(&mut app, "fri");
    enter(&mut app);
    assert_lacks(&mut app, "Filter:");
    assert_rendered(&mut app, &SPEC, &["frieren.png"]);
}

#[test]
fn enter_on_a_filtered_row_applies_that_wallpaper_not_the_same_index() {
    // Catches: index desync between the filtered view and the full list. After filter `c1` the
    // only row is c1.png (filtered index 0); the buggy code would apply a1.png (unfiltered 0).
    let (mut app, sys, dir) = focused("s12-apply-first", &ABC);
    press(&mut app, '/');
    type_str(&mut app, "c1");
    enter(&mut app); // closes the input
    assert!(sys.spawn_calls().is_empty(), "closing must not apply");
    enter(&mut app); // applies the highlighted filtered row
    assert_eq!(sys.spawn_calls(), vec![spawn_for(&dir, "c1.png")]);
    assert_app_status(&mut app, "Applying c1.png…");
}

#[test]
fn j_moves_within_the_filtered_rows_and_enter_applies_the_highlighted_one() {
    // Catches: j/k indexing the unfiltered list. Filter `b` leaves b1 (filtered 0) and b2
    // (filtered 1, unfiltered 2); `j` must land on b2 and Enter must apply b2.
    let (mut app, sys, dir) = focused("s12-apply-j", &ABC);
    press(&mut app, '/');
    press(&mut app, 'b');
    enter(&mut app);
    assert_rendered(&mut app, &ABC, &["b1.png", "b2.png"]);
    assert_highlighted(&mut app, &["b1.png", "b2.png"], "b1.png");
    press(&mut app, 'j');
    assert_highlighted(&mut app, &["b1.png", "b2.png"], "b2.png");
    press(&mut app, 'j'); // clamps at the last filtered row
    assert_highlighted(&mut app, &["b1.png", "b2.png"], "b2.png");
    enter(&mut app);
    assert_eq!(sys.spawn_calls(), vec![spawn_for(&dir, "b2.png")]);

    press(&mut app, 'k');
    assert_highlighted(&mut app, &["b1.png", "b2.png"], "b1.png");
    press(&mut app, 'k'); // clamps at the first filtered row
    assert_highlighted(&mut app, &["b1.png", "b2.png"], "b1.png");
}

#[test]
fn quit_key_quits_again_once_the_input_is_closed() {
    // Catches: a closed filter still swallowing keys, so `q` can no longer quit.
    let (mut app, _sys, _dir) = focused("s12-quit", &ABC);
    press(&mut app, '/');
    press(&mut app, 'b');
    enter(&mut app);
    assert_rendered(&mut app, &ABC, &["b1.png", "b2.png"]);
    assert!(!app.should_quit());
    press(&mut app, 'q');
    assert!(app.should_quit());
}

// ---- Esc ----

#[test]
fn esc_while_typing_clears_the_filter_and_keeps_the_highlighted_wallpaper() {
    // Catches: Esc leaving the filter active, closing only the input, or resetting the cursor.
    // After filter `c` the highlight is c1.png (filtered 0, unfiltered 3).
    let (mut app, _sys, _dir) = focused("s12-esc-typing", &ABC);
    press(&mut app, '/');
    press(&mut app, 'c');
    assert_rendered(&mut app, &ABC, &["c1.png"]);
    esc(&mut app);
    assert_lacks(&mut app, "Filter:");
    assert_rendered(&mut app, &ABC, &ABC);
    assert_highlighted(&mut app, &ABC, "c1.png");
}

#[test]
fn esc_with_an_active_closed_filter_clears_it_and_keeps_the_highlight() {
    // Catches: Esc doing nothing once the input is closed. The highlight b2.png has filtered
    // index 1 but unfiltered index 2.
    let (mut app, _sys, _dir) = focused("s12-esc-closed", &ABC);
    press(&mut app, '/');
    press(&mut app, 'b');
    enter(&mut app);
    assert_rendered(&mut app, &ABC, &["b1.png", "b2.png"]);
    press(&mut app, 'j');
    assert_highlighted(&mut app, &["b1.png", "b2.png"], "b2.png");
    esc(&mut app);
    assert_rendered(&mut app, &ABC, &ABC);
    assert_highlighted(&mut app, &ABC, "b2.png");
}

// ---- no matches ----

#[test]
fn zero_matches_render_no_rows_no_preview_and_apply_nothing() {
    // Catches: panics on an empty filtered list, stale rows or a stale preview name, and Enter
    // applying some wallpaper when nothing is highlighted.
    let (mut app, sys, _dir) = focused("s12-zero", &SPEC);
    press(&mut app, '/');
    type_str(&mut app, "zzz");
    enter(&mut app); // closes the input
    let buf = draw(&mut app);
    assert!(rendered(&buf, &SPEC).is_empty(), "{}", dump(&buf));
    for n in SPEC {
        assert!(
            !find_text(&buf, n)
                .iter()
                .any(|&(x, _)| x >= PREVIEW_X_START),
            "preview must not name `{n}`; buffer:\n{}",
            dump(&buf)
        );
    }
    press(&mut app, 'j');
    press(&mut app, 'k');
    press(&mut app, 'j');
    enter(&mut app);
    assert!(sys.spawn_calls().is_empty(), "{:?}", sys.spawn_calls());
    assert_rendered(&mut app, &SPEC, &[]);
}

// ---- current mark ----

#[test]
fn current_mark_stays_on_the_filtered_applied_row() {
    // Catches: the `●` mark dropped or moved when rows are filtered. The applied wallpaper is
    // b2.png; with filter `b` it is the second of two rows.
    let (mut app, _sys, _dir) = setup("s12-mark", &ABC, Some("b2.png"));
    press(&mut app, 'l');
    press(&mut app, '/');
    press(&mut app, 'b');
    enter(&mut app);
    assert_rendered(&mut app, &ABC, &["b1.png", "b2.png"]);
    let buf = draw(&mut app);
    let b2_y = row_of(&buf, "b2.png", ITEMS_X_START, ITEMS_X_END);
    let marks: Vec<u16> = items_hits(&buf, "●", ITEMS_X_START, ITEMS_X_END)
        .into_iter()
        .map(|(_, y)| y)
        .collect();
    assert_eq!(marks, vec![b2_y], "buffer:\n{}", dump(&buf));
}

// ---- leaving the items column ----

#[test]
fn leaving_the_items_column_clears_the_filter() {
    // Catches: the filter surviving `h` + `l`, so the refocused list is still narrowed.
    let (mut app, _sys, _dir) = focused("s12-leave", &ABC);
    press(&mut app, '/');
    press(&mut app, 'b');
    enter(&mut app);
    assert_rendered(&mut app, &ABC, &["b1.png", "b2.png"]);
    press(&mut app, 'h');
    press(&mut app, 'l');
    assert_rendered(&mut app, &ABC, &ABC);
    assert!(!shows(&mut app, "Filter:"));
}

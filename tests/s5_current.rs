//! S5-current-wallpaper: the applied wallpaper is marked with `●` and preselected.
//!
//! Highlight observable (same idea as S1): the "style" of an items row is the triple
//! (fg, bg, add_modifier) of the cell holding the FIRST letter of the wallpaper name inside the
//! items column (x in 20..40 at 80 columns). Exactly one row is highlighted: its first-letter
//! style differs from every other row's, and the non-highlighted rows share one style.
//! The first letter must lie inside the highlighted span (style the whole row, e.g. ratatui
//! `highlight_style`), and the `●` marker must not change the plain rows' name style.

mod common;

use std::fs;
use std::path::Path;

use common::*;
use ln_hub::{App, Config};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;

const W: u16 = 80;
const H: u16 = 24;
const ITEMS_X_START: u16 = 20;
const ITEMS_X_END: u16 = 40;
const NAMES: [&str; 3] = ["img1.png", "img2.png", "img3.png"];

fn touch(path: &Path) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, b"x").unwrap();
}

/// Wallpaper dir with img1..img3; returns (cfg, dir). The current-wallpaper file is NOT created.
fn setup(name: &str) -> (Config, std::path::PathBuf) {
    let home = tempdir(&format!("{name}-home"));
    let dir = tempdir(&format!("{name}-wall"));
    for n in NAMES {
        touch(&dir.join(n));
    }
    let mut cfg = Config::default_for(&home);
    cfg.wallpaper_dir = dir.clone();
    (cfg, dir)
}

/// Builds the App, focuses Theme's items with `l`, renders at 80x24.
fn focus_and_draw(cfg: Config) -> Buffer {
    let mut app = App::new(cfg, Box::new(FakeSystem::new()));
    app.handle_key(key(KeyCode::Char('l')));
    let mut term = Terminal::new(TestBackend::new(W, H)).unwrap();
    term.draw(|f| app.render(f)).unwrap();
    term.backend().buffer().clone()
}

/// Asserts exactly `expected` is highlighted among img1..img3 (see module docs).
fn assert_only_highlighted(buf: &Buffer, expected: &str) {
    common::assert_only_highlighted(buf, &NAMES, expected, |b, n| {
        items_hits(b, n, ITEMS_X_START, ITEMS_X_END)
            .first()
            .copied()
    });
}

#[test]
fn current_wallpaper_is_marked_and_preselected() {
    // Catches: current_wallpaper file ignored (no `●` on the matching row, or `●` on the wrong
    // row), or the cursor left on the first item instead of the current wallpaper.
    let (mut cfg, dir) = setup("s5-current");
    let cur = tempdir("s5-current-cache").join("current_wallpaper");
    fs::write(&cur, format!("{}/img2.png\n", dir.display())).unwrap();
    cfg.current_wallpaper_file = cur;
    let buf = focus_and_draw(cfg);

    let (_, img2_y) = name_cell(&buf, "img2.png", ITEMS_X_START, ITEMS_X_END);
    let marks = items_hits(&buf, "●", ITEMS_X_START, ITEMS_X_END);
    assert_eq!(
        marks.iter().map(|&(_, y)| y).collect::<Vec<_>>(),
        vec![img2_y],
        "exactly one `●`, on the img2.png row (y={img2_y}); buffer:\n{}",
        dump(&buf)
    );
    assert_only_highlighted(&buf, "img2.png");
}

#[test]
fn missing_current_file_has_no_marker_and_highlights_first_item() {
    // Catches: a `●` drawn without a current wallpaper, or no highlight / wrong row highlighted
    // when the current_wallpaper file is absent (must default to the first item).
    let (cfg, _dir) = setup("s5-missing");
    assert!(!cfg.current_wallpaper_file.exists());
    let buf = focus_and_draw(cfg);

    assert!(
        items_hits(&buf, "●", ITEMS_X_START, ITEMS_X_END).is_empty(),
        "no `●` expected without a current_wallpaper file; buffer:\n{}",
        dump(&buf)
    );
    assert_only_highlighted(&buf, "img1.png");
}

#[test]
fn current_path_outside_list_has_no_marker_and_highlights_first_item() {
    // Catches: matching the current wallpaper by file name instead of full path (a current path
    // `/elsewhere/img2.png` outside wallpaper_dir would wrongly mark and preselect listed img2.png).
    let (mut cfg, _dir) = setup("s5-elsewhere");
    let cur = tempdir("s5-elsewhere-cache").join("current_wallpaper");
    fs::write(&cur, "/elsewhere/img2.png\n").unwrap();
    cfg.current_wallpaper_file = cur;
    let buf = focus_and_draw(cfg);

    assert!(
        items_hits(&buf, "●", ITEMS_X_START, ITEMS_X_END).is_empty(),
        "no `●` expected when the current path is not in the list; buffer:\n{}",
        dump(&buf)
    );
    assert_only_highlighted(&buf, "img1.png");
}

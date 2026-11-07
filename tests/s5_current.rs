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
use ratatui::style::{Color, Modifier};

const W: u16 = 80;
const H: u16 = 24;
const ITEMS_X_START: u16 = 20;
const ITEMS_X_END: u16 = 40;
const NAMES: [&str; 3] = ["img1.png", "img2.png", "img3.png"];

type Style = (Color, Color, Modifier);

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

fn items_hits(buf: &Buffer, text: &str) -> Vec<(u16, u16)> {
    find_text(buf, text)
        .into_iter()
        .filter(|&(x, _)| (ITEMS_X_START..ITEMS_X_END).contains(&x))
        .collect()
}

fn dump(buf: &Buffer) -> String {
    buffer_lines(buf).join("\n")
}

fn name_pos(buf: &Buffer, name: &str) -> (u16, u16) {
    *items_hits(buf, name).first().unwrap_or_else(|| {
        panic!("`{name}` not found in items column; buffer:\n{}", dump(buf))
    })
}

fn name_style(buf: &Buffer, name: &str) -> Style {
    let (x, y) = name_pos(buf, name);
    let c = &buf[(x, y)];
    (c.fg, c.bg, c.modifier)
}

/// Asserts exactly `expected` is highlighted among img1..img3 (see module docs).
fn assert_only_highlighted(buf: &Buffer, expected: &str) {
    let styles: Vec<(&str, Style)> = NAMES.iter().map(|n| (*n, name_style(buf, n))).collect();
    let others: Vec<&(&str, Style)> = styles.iter().filter(|(n, _)| *n != expected).collect();
    let hl = styles.iter().find(|(n, _)| *n == expected).unwrap().1;
    assert!(
        others.iter().all(|(_, s)| *s == others[0].1),
        "non-highlighted rows must share one style, got {styles:?}; buffer:\n{}",
        dump(buf)
    );
    assert_ne!(
        hl, others[0].1,
        "`{expected}` must be the highlighted row (style must differ from the others), got {styles:?}; buffer:\n{}",
        dump(buf)
    );
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

    let (_, img2_y) = name_pos(&buf, "img2.png");
    let marks = items_hits(&buf, "●");
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
        items_hits(&buf, "●").is_empty(),
        "no `●` expected without a current_wallpaper file; buffer:\n{}",
        dump(&buf)
    );
    assert_only_highlighted(&buf, "img1.png");
}

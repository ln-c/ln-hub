//! S1-shell-tracer: three-column shell, module list, `q` quits, `j` moves highlight.
//!
//! Highlight observable (contract for the implementer): the "style" of a module row is the
//! triple (fg, bg, add_modifier) of the cell holding the FIRST letter of the module name
//! ("T" of `Theme`, "M" of `Monitors`) in the leftmost column (x < 80/3). Exactly one of the
//! two module rows is highlighted: its first-letter cell style differs from the other row's.
//! No specific colours/modifiers are prescribed, but the non-highlighted rows' style must be
//! stable, i.e. after `j` the Theme row has the style Monitors had before, and vice versa.
//! The first letter must be inside the highlighted span (e.g. use ratatui `highlight_style`
//! on a List, or style the whole row; do not style only the marker prefix).

mod common;

use common::*;
use ln_hub::{App, Config};
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::style::{Color, Modifier};
use ratatui::Terminal;

const W: u16 = 80;
const H: u16 = 24;
const LEFT_COL_MAX_X: u16 = W / 3; // leftmost column = x < 26

fn new_app(name: &str) -> App {
    let tmp = tempdir(name);
    App::new(Config::default_for(&tmp), Box::new(FakeSystem::new()))
}

fn draw(app: &mut App) -> Buffer {
    let mut term = Terminal::new(TestBackend::new(W, H)).unwrap();
    term.draw(|f| app.render(f)).unwrap();
    term.backend().buffer().clone()
}

/// Position of `label` in the leftmost column, panicking with the dump if absent.
fn left_pos(buf: &Buffer, label: &str) -> (u16, u16) {
    find_text(buf, label)
        .into_iter()
        .find(|&(x, _)| x < LEFT_COL_MAX_X)
        .unwrap_or_else(|| {
            panic!(
                "`{label}` not found in leftmost column (x < {LEFT_COL_MAX_X}); buffer:\n{}",
                buffer_lines(buf).join("\n")
            )
        })
}

type Style = (Color, Color, Modifier);

fn style_at(buf: &Buffer, (x, y): (u16, u16)) -> Style {
    let c = &buf[(x, y)];
    (c.fg, c.bg, c.modifier)
}

fn module_styles(buf: &Buffer) -> (Style, Style) {
    (
        style_at(buf, left_pos(buf, "Theme")),
        style_at(buf, left_pos(buf, "Monitors")),
    )
}

#[test]
fn modules_listed_theme_above_monitors_in_leftmost_column() {
    // Catches: modules not rendered, wrong order, or rendered outside the left column.
    let mut app = new_app("layout");
    let buf = draw(&mut app);
    let (_, theme_y) = left_pos(&buf, "Theme");
    let (_, mon_y) = left_pos(&buf, "Monitors");
    assert!(
        theme_y < mon_y,
        "Theme (row {theme_y}) must be above Monitors (row {mon_y})"
    );
}

#[test]
fn q_quits() {
    // Catches: `q` not wired to quit, or app quitting before any key.
    let mut app = new_app("quit");
    assert!(!app.should_quit(), "must not quit before any key");
    app.handle_key(key(KeyCode::Char('q')));
    assert!(app.should_quit(), "must quit after `q`");
}

#[test]
fn j_moves_highlight_theme_to_monitors_without_wrap() {
    // Catches: no highlight, highlight not moving on `j`, or wrapping past the last module.
    let mut app = new_app("highlight");

    let (theme0, mon0) = module_styles(&draw(&mut app));
    assert_ne!(theme0, mon0, "initially exactly Theme must be highlighted (styles differ)");

    app.handle_key(key(KeyCode::Char('j')));
    let (theme1, mon1) = module_styles(&draw(&mut app));
    assert_eq!(mon1, theme0, "after j, Monitors must carry the highlight style Theme had");
    assert_eq!(theme1, mon0, "after j, Theme must carry the plain style Monitors had");
    assert_ne!(theme1, mon1, "after j, highlight must still differ between rows");

    app.handle_key(key(KeyCode::Char('j')));
    let (theme2, mon2) = module_styles(&draw(&mut app));
    assert_eq!((theme2, mon2), (theme1, mon1), "second j must stay on Monitors (no wrap)");
}

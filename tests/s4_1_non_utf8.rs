//! S4.1-wallpaper-list-hardening: a wallpaper whose file name is not valid UTF-8 is skipped.
//!
//! At 80 columns the items column spans x in 20..40.

mod common;

use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStrExt;

use common::*;
use ln_hub::{App, Config};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::KeyCode;

const W: u16 = 80;
const H: u16 = 24;
const ITEMS_X_START: u16 = 20;
const ITEMS_X_END: u16 = 40;

#[test]
fn non_utf8_file_name_is_skipped_but_neighbour_is_listed() {
    // Catches: listing a non-UTF-8 name lossily (`bad\u{FFFD}name.png`), whose path cannot
    // round-trip through the display name, instead of skipping it.
    let home = tempdir("s41-nonutf8-home");
    let dir = tempdir("s41-nonutf8-wall");
    fs::write(dir.join("good.png"), b"x").unwrap();
    fs::write(dir.join(OsStr::from_bytes(b"bad\xffname.png")), b"x").unwrap();

    let mut cfg = Config::default_for(&home);
    cfg.wallpaper_dir = dir;
    let mut app = App::new(cfg, Box::new(FakeSystem::new()));
    app.handle_key(key(KeyCode::Char('l')));
    let mut term = Terminal::new(TestBackend::new(W, H)).unwrap();
    term.draw(|f| app.render(f)).unwrap();
    let buf = term.backend().buffer().clone();

    let column_rows: Vec<String> = (0..H)
        .map(|y| {
            (ITEMS_X_START..ITEMS_X_END)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .collect();
    let dump = column_rows.join("\n");

    assert!(
        column_rows.iter().any(|r| r.contains("good.png")),
        "good.png must be listed in the items column; column:\n{dump}"
    );
    for row in &column_rows {
        assert!(
            !row.contains("bad") && !row.contains('\u{FFFD}'),
            "non-UTF-8 file must be skipped, found row `{row}`; column:\n{dump}"
        );
    }
}

//! S4-wallpaper-list: the Theme module lists wallpapers in the items column.
//!
//! Layout contract (from the shell): Length(20) | Fill(1) | Fill(2). At 80 columns the items
//! column spans x in 20..40; at 300 columns it spans x in 20..~93 (we only rely on 20..90).

mod common;

use std::fs;
use std::path::Path;

use common::*;
use ln_hub::{App, Config};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;

const H: u16 = 24;
const ITEMS_X_START: u16 = 20;

fn touch(path: &Path) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, b"x").unwrap();
}

/// Builds an App (after the caller created its files), focuses Theme's items with `l`,
/// and renders at `width` x 24.
fn focus_and_draw(cfg: Config, width: u16) -> Buffer {
    let mut app = App::new(cfg, Box::new(FakeSystem::new()));
    app.handle_key(key(KeyCode::Char('l')));
    let mut term = Terminal::new(TestBackend::new(width, H)).unwrap();
    term.draw(|f| app.render(f)).unwrap();
    term.backend().buffer().clone()
}

#[test]
fn lists_sorted_naturally_hides_thumbs_and_non_media() {
    // Catches: unsorted/lexicographic order (img10 before img2), missing subdir-last ordering,
    // video thumbnail b.jpg shown, or non-media notes.txt shown.
    let home = tempdir("s4-order-home");
    let dir = tempdir("s4-order-wall");
    for f in [
        "img10.png",
        "img2.png",
        "b.mp4",
        "b.jpg",
        "notes.txt",
        "images/c.jpg",
    ] {
        touch(&dir.join(f));
    }
    let mut cfg = Config::default_for(&home);
    cfg.wallpaper_dir = dir;
    let buf = focus_and_draw(cfg, 80);
    let end = 40;

    let mp4 = row_of(&buf, "b.mp4", ITEMS_X_START, end);
    let img2 = row_of(&buf, "img2.png", ITEMS_X_START, end);
    let img10 = row_of(&buf, "img10.png", ITEMS_X_START, end);
    let sub = row_of(&buf, "images/c.jpg", ITEMS_X_START, end);
    assert!(
        mp4 < img2,
        "b.mp4 (row {mp4}) must precede img2.png (row {img2})"
    );
    assert!(
        img2 < img10,
        "img2.png (row {img2}) must precede img10.png (row {img10})"
    );
    assert!(
        img10 < sub,
        "img10.png (row {img10}) must precede images/c.jpg (row {sub})"
    );

    assert!(
        items_hits(&buf, "b.jpg", ITEMS_X_START, end).is_empty(),
        "video thumbnail b.jpg must be hidden; buffer:\n{}",
        buffer_lines(&buf).join("\n")
    );
    assert!(
        items_hits(&buf, "notes.txt", ITEMS_X_START, end).is_empty(),
        "notes.txt must be hidden; buffer:\n{}",
        buffer_lines(&buf).join("\n")
    );
}

fn assert_no_wallpapers_message(dir: &Path, name: &str) {
    let home = tempdir(name);
    let mut cfg = Config::default_for(&home);
    cfg.wallpaper_dir = dir.to_path_buf();
    // 300 wide: items column is ~73 cells (x in 20..93), wide enough for the full message.
    let buf = focus_and_draw(cfg, 300);
    let msg = format!("No wallpapers in {}", dir.display());
    assert!(
        !items_hits(&buf, &msg, ITEMS_X_START, 93).is_empty(),
        "expected `{msg}` in items column; buffer:\n{}",
        buffer_lines(&buf).join("\n")
    );
}

#[test]
fn empty_dir_shows_no_wallpapers_message() {
    // Catches: empty wallpaper dir rendering a blank items column (or panicking).
    let dir = tempdir("s4-empty-wall");
    assert_no_wallpapers_message(&dir, "s4-empty-home");
}

#[test]
fn missing_dir_shows_no_wallpapers_message() {
    // Catches: nonexistent wallpaper dir panicking or rendering a blank items column.
    let dir = tempdir("s4-missing-parent").join("does-not-exist");
    assert!(!dir.exists());
    assert_no_wallpapers_message(&dir, "s4-missing-home");
}

#[test]
fn default_config_lists_pictures_wallpapers_under_home() {
    // Catches: App ignoring cfg.wallpaper_dir (deferred S3 check): default_for(home) must
    // surface files in <home>/Pictures/Wallpapers.
    let home = tempdir("s4-default-home");
    touch(&home.join("Pictures/Wallpapers/img2.png"));
    let buf = focus_and_draw(Config::default_for(&home), 80);
    assert!(
        !items_hits(&buf, "img2.png", ITEMS_X_START, 40).is_empty(),
        "img2.png must be listed in items column; buffer:\n{}",
        buffer_lines(&buf).join("\n")
    );
}

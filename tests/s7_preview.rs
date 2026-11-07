//! S7-preview-meta: the preview column (x >= 40 on an 80x24 buffer) shows details of the wallpaper
//! under the items cursor. Only the bin draws the image itself, so tests check text only.
//!
//! All assertions are restricted to the preview column because the items column (x 20..40) also
//! shows wallpaper names.

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
const PREVIEW_X_START: u16 = 40;

fn write_bytes(dir: &Path, name: &str, len: usize) {
    fs::write(dir.join(name), vec![b'x'; len]).unwrap();
}

/// Builds an App over a fresh wallpaper dir populated by `fill`, focused on the items column.
fn app_with(name: &str, fill: impl FnOnce(&Path)) -> App {
    let home = tempdir(&format!("{name}-home"));
    let dir = tempdir(&format!("{name}-wall"));
    fill(&dir);
    let mut cfg = Config::default_for(&home);
    cfg.wallpaper_dir = dir;
    let mut app = App::new(cfg, Box::new(FakeSystem::new()));
    app.handle_key(key(KeyCode::Char('l')));
    app
}

fn press(app: &mut App, c: char) {
    app.handle_key(key(KeyCode::Char(c)));
}

fn draw(app: &mut App) -> Buffer {
    let mut term = Terminal::new(TestBackend::new(W, H)).unwrap();
    term.draw(|f| app.render(f)).unwrap();
    term.backend().buffer().clone()
}

fn preview_has(buf: &Buffer, text: &str) -> bool {
    find_text(buf, text)
        .iter()
        .any(|&(x, _)| x >= PREVIEW_X_START)
}

fn dump(buf: &Buffer) -> String {
    buffer_lines(buf).join("\n")
}

fn assert_preview_has(buf: &Buffer, text: &str) {
    assert!(
        preview_has(buf, text),
        "preview column must show `{text}`; buffer:\n{}",
        dump(buf)
    );
}

fn assert_preview_lacks(buf: &Buffer, text: &str) {
    assert!(
        !preview_has(buf, text),
        "preview column must not show `{text}`; buffer:\n{}",
        dump(buf)
    );
}

fn two_images(dir: &Path) {
    write_bytes(dir, "img1.png", 1024);
    write_bytes(dir, "img2.png", 2048);
}

// Breaks if the preview stays an empty "Preview" box, or always shows the first item's details.
#[test]
fn hovering_img2_shows_its_name_and_size_in_the_preview() {
    let mut app = app_with("s7-img2", two_images);
    press(&mut app, 'j'); // cursor: img1.png -> img2.png
    let buf = draw(&mut app);
    assert_preview_has(&buf, "img2.png");
    assert_preview_has(&buf, "2.0K");
    assert_preview_lacks(&buf, "img1.png");
    assert_preview_lacks(&buf, "1.0K");
}

// Breaks if the preview does not follow the items cursor when it moves down and back up.
#[test]
fn preview_follows_the_cursor() {
    let mut app = app_with("s7-follow", two_images);
    let buf = draw(&mut app);
    assert_preview_has(&buf, "img1.png");
    assert_preview_has(&buf, "1.0K");
    assert_preview_lacks(&buf, "2.0K");

    press(&mut app, 'j');
    let buf = draw(&mut app);
    assert_preview_has(&buf, "2.0K");
    assert_preview_lacks(&buf, "1.0K");

    press(&mut app, 'k');
    let buf = draw(&mut app);
    assert_preview_has(&buf, "img1.png");
    assert_preview_has(&buf, "1.0K");
    assert_preview_lacks(&buf, "2.0K");
}

// Breaks if a hovered video does not show its kind (`video`) and its paired thumbnail name.
#[test]
fn hovering_a_video_shows_video_and_its_thumb() {
    let mut app = app_with("s7-video", |dir| {
        write_bytes(dir, "b.mp4", 10);
        write_bytes(dir, "b.jpg", 10);
        write_bytes(dir, "img1.png", 10);
    });
    // Items are sorted: b.mp4 (b.jpg hidden as its thumb), img1.png. Cursor starts on b.mp4.
    let buf = draw(&mut app);
    assert_preview_has(&buf, "video");
    assert_preview_has(&buf, "thumb: b.jpg");
}

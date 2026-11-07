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
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;

const PREVIEW_X_START: u16 = 40;

fn write_bytes(dir: &Path, name: &str, len: usize) {
    fs::write(dir.join(name), vec![b'x'; len]).unwrap();
}

/// Builds an App over a fresh wallpaper dir populated by `fill`, items column NOT focused.
fn unfocused_app_with(name: &str, fill: impl FnOnce(&Path)) -> App {
    let home = tempdir(&format!("{name}-home"));
    let dir = tempdir(&format!("{name}-wall"));
    fill(&dir);
    let mut cfg = Config::default_for(&home);
    cfg.wallpaper_dir = dir;
    App::new(cfg, Box::new(FakeSystem::new()))
}

/// Builds an App over a fresh wallpaper dir populated by `fill`, focused on the items column.
fn app_with(name: &str, fill: impl FnOnce(&Path)) -> App {
    let mut app = unfocused_app_with(name, fill);
    app.handle_key(key(KeyCode::Char('l')));
    app
}

/// Creates a sparse file of `len` bytes (cheap even for large sizes).
fn write_sparse(dir: &Path, name: &str, len: u64) {
    fs::File::create(dir.join(name))
        .unwrap()
        .set_len(len)
        .unwrap();
}

/// Interior text of each preview row (between the borders), right-trimmed.
fn preview_rows(buf: &Buffer) -> Vec<String> {
    (0..buf.area.height)
        .map(|y| {
            ((PREVIEW_X_START + 1)..(buf.area.width - 1))
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect()
}

/// Asserts some preview row is exactly `text` (so `10` cannot match inside other text).
fn assert_preview_row(buf: &Buffer, text: &str) {
    assert!(
        preview_rows(buf).iter().any(|r| r == text),
        "preview must have a row exactly `{text}`; buffer:\n{}",
        dump(buf)
    );
}

fn preview_has(buf: &Buffer, text: &str) -> bool {
    find_text(buf, text)
        .iter()
        .any(|&(x, _)| x >= PREVIEW_X_START)
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

/// Hovers a single file of `len` bytes and asserts the preview's size row.
fn assert_size_shown(len: u64, expected: &str) {
    let mut app = app_with(&format!("s7-size-{len}"), |dir| {
        write_sparse(dir, "a.png", len)
    });
    let buf = draw(&mut app);
    assert_preview_row(&buf, expected);
}

// Breaks if sizes below 1 KiB are not shown as a plain byte count.
#[test]
fn size_10_bytes_is_plain() {
    assert_size_shown(10, "10");
}

// Breaks if 1500 bytes is not shown as `1.5K` (one decimal under 10 units).
#[test]
fn size_1500_is_1_5k() {
    assert_size_shown(1500, "1.5K");
}

// Breaks if sizes round down (1025 bytes -> `1.0K`) instead of up (`1.1K`), as `ls -h` does.
#[test]
fn size_1025_rounds_up_to_1_1k() {
    assert_size_shown(1025, "1.1K");
}

// Breaks if sizes of 10 units or more keep a decimal (10240 -> `10K`, not `10.0K`).
#[test]
fn size_10240_is_10k() {
    assert_size_shown(10240, "10K");
}

// Breaks if the unit does not advance past K (1 MiB -> `1.0M`).
#[test]
fn size_one_mib_is_1_0m() {
    assert_size_shown(1_048_576, "1.0M");
}

// Breaks if an image is labelled with the wrong kind, or shows a thumb line it does not have.
#[test]
fn hovering_an_image_shows_image_and_no_thumb() {
    let mut app = app_with("s7-image-kind", |dir| write_bytes(dir, "a.png", 10));
    let buf = draw(&mut app);
    assert_preview_row(&buf, "image");
    assert_preview_lacks(&buf, "video");
    assert_preview_lacks(&buf, "thumb:");
}

// Breaks if the preview shows wallpaper details before the items column is focused.
#[test]
fn unfocused_preview_shows_no_details() {
    let mut app = unfocused_app_with("s7-unfocused", |dir| {
        write_bytes(dir, "img1.png", 1024);
        write_bytes(dir, "img2.png", 2048);
    });
    let buf = draw(&mut app);
    assert_preview_lacks(&buf, "img1.png");
    assert_preview_lacks(&buf, "1.0K");
    assert_preview_lacks(&buf, "image");
}

// Breaks if an empty wallpaper list panics the preview or renders placeholder/size/kind text.
#[test]
fn empty_list_preview_is_blank() {
    let mut app = app_with("s7-empty", |_| {});
    let buf = draw(&mut app);
    assert_preview_lacks(&buf, "image");
    assert_preview_lacks(&buf, "video");
    assert_preview_lacks(&buf, "thumb:");
    // Only the top row (border + title) and bottom border/status may be non-blank; interior is blank.
    let rows = preview_rows(&buf);
    let non_blank: Vec<_> = rows[1..rows.len() - 2]
        .iter()
        .filter(|r| !r.is_empty())
        .collect();
    assert!(
        non_blank.is_empty(),
        "empty-list preview interior must be blank, got {non_blank:?}; buffer:\n{}",
        dump(&buf)
    );
}

// Breaks if sizes of 10 units or more round down (10241 bytes -> `10K`) instead of up (`11K`).
#[test]
fn size_10241_rounds_up_to_11k() {
    assert_size_shown(10241, "11K");
}

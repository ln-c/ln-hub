//! S13-preview-image: `App::preview_image()` tells the binary which image to draw as the sixel
//! preview and where. It reflects the LAST `App::render`: the file under the Theme items cursor
//! (a video's paired `.jpg` thumbnail), placed strictly inside the Preview block border and below
//! the detail text lines. `None` whenever there is nothing to draw.
//!
//! Layout facts (80x24): the preview column is x 40..80 with its right border at x = 79; the
//! status row is the last row, so the Preview block's bottom border is at row height - 2.

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::*;
use ln_hub::{App, Config, PreviewImage};
use ratatui::buffer::Buffer;

const PREVIEW_X_START: u16 = 40;

/// App over a fresh wallpaper dir populated by `fill`; the items column is focused (`l`).
/// Returns the app and the wallpaper dir.
fn focused_app(name: &str, fill: impl FnOnce(&Path)) -> (App, PathBuf) {
    let home = tempdir(&format!("{name}-home"));
    let dir = tempdir(&format!("{name}-wall"));
    fill(&dir);
    let mut cfg = Config::default_for(&home);
    cfg.wallpaper_dir = dir.clone();
    let mut app = App::new(cfg, Box::new(FakeSystem::new()));
    press(&mut app, 'l');
    (app, dir)
}

fn write(dir: &Path, name: &str) {
    fs::write(dir.join(name), b"x").unwrap();
}

fn type_str(app: &mut App, s: &str) {
    for c in s.chars() {
        press(app, c);
    }
}

fn png(dir: &Path) {
    write(dir, "a.png");
}

fn three_pngs(dir: &Path) {
    for n in ["a1.png", "b1.png", "c1.png"] {
        write(dir, n);
    }
}

/// Row of `text` inside the preview column; panics with a dump when absent.
fn preview_row(buf: &Buffer, text: &str) -> u16 {
    find_text(buf, text)
        .into_iter()
        .find(|&(x, _)| x >= PREVIEW_X_START)
        .unwrap_or_else(|| panic!("`{text}` not in preview column; buffer:\n{}", dump(buf)))
        .1
}

/// Draws at `w` x `h` and returns the image plus the buffer.
fn image_at(app: &mut App, w: u16, h: u16) -> (Option<PreviewImage>, Buffer) {
    let buf = draw_sized(app, w, h);
    (app.preview_image(), buf)
}

/// Asserts the area lies strictly inside the Preview block border and below `last_text_row`.
fn assert_placed(img: &PreviewImage, buf: &Buffer, last_text_row: u16) {
    let (title_x, block_top) = find_text(buf, "Preview")
        .into_iter()
        .find(|&(x, _)| x >= PREVIEW_X_START)
        .unwrap_or_else(|| panic!("no Preview title; buffer:\n{}", dump(buf)));
    let block_left = title_x - 1; // the title starts one cell right of the left border
    let (w, h) = (buf.area.width, buf.area.height);
    let a = img.area;
    let ctx = format!("area {a:?}; buffer:\n{}", dump(buf));
    assert!(a.width > 0 && a.height > 0, "area must be non-empty, {ctx}");
    assert!(
        a.x > block_left,
        "area.x must be right of the left border, {ctx}"
    );
    assert!(
        a.right() <= w - 1,
        "area must stay left of the right border, {ctx}"
    );
    assert!(a.y > block_top, "area must be below the top border, {ctx}");
    assert!(
        a.y > last_text_row,
        "area must start below the last text row {last_text_row}, {ctx}"
    );
    assert!(
        a.bottom() <= h - 2,
        "area must stay above the bottom border (row {}), {ctx}",
        h - 2
    );
}

// Breaks if the preview image is never offered, or points at another file than the hovered image.
#[test]
fn hovered_image_is_the_preview_path_and_sits_below_the_text() {
    let (mut app, dir) = focused_app("s13pi-image", png);
    let (img, buf) = image_at(&mut app, 80, 24);
    let img = img.unwrap_or_else(|| panic!("preview_image must be Some; buffer:\n{}", dump(&buf)));
    assert_eq!(img.path, dir.join("a.png"));
    // Text lines of an image: name, size, `image`; the last one is `image`.
    assert_placed(&img, &buf, preview_row(&buf, "image"));
}

// Breaks if a hovered video offers the video file (not decodable as an image) instead of its
// paired `.jpg` thumbnail, or the area ignores the extra `thumb:` text line.
#[test]
fn hovered_video_uses_its_thumbnail_path() {
    let (mut app, dir) = focused_app("s13pi-video", |d| {
        write(d, "b.mp4");
        write(d, "b.jpg");
        write(d, "img1.png");
    });
    // Items: b.mp4 (b.jpg is its hidden thumb), img1.png; the cursor starts on b.mp4.
    let (img, buf) = image_at(&mut app, 80, 24);
    let img = img.unwrap_or_else(|| panic!("preview_image must be Some; buffer:\n{}", dump(&buf)));
    assert_eq!(img.path, dir.join("b.jpg"));
    // Text lines: b.mp4, size, `video`, `thumb: b.jpg`; the last one is the thumb line.
    assert_placed(&img, &buf, preview_row(&buf, "thumb: b.jpg"));
}

// Breaks if a video without a thumbnail offers the video file (or anything) as the image.
#[test]
fn video_without_a_thumbnail_has_no_image() {
    let (mut app, _dir) = focused_app("s13pi-nothumb", |d| write(d, "c.mp4"));
    let (img, buf) = image_at(&mut app, 80, 24);
    assert!(img.is_none(), "got {img:?}; buffer:\n{}", dump(&buf));
    assert!(preview_row(&buf, "video") > 0, "video text is shown");
}

// Breaks if the area is returned even when the detail text fills the whole block interior
// (image needs rows below the 3 text lines: 6 rows -> interior 3 rows, no room left).
#[test]
fn no_image_rows_left_below_the_text_gives_none() {
    let (mut app, dir) = focused_app("s13pi-small", png);
    let (img, buf) = image_at(&mut app, 80, 6);
    assert_eq!(
        preview_row(&buf, "image"),
        3,
        "text fills the interior; {}",
        dump(&buf)
    );
    assert!(img.is_none(), "got {img:?}; buffer:\n{}", dump(&buf));
    // Same app with room: the image is offered (so None above is about space, not the fixture).
    let (img, buf) = image_at(&mut app, 80, 24);
    assert_eq!(
        img.map(|i| i.path),
        Some(dir.join("a.png")),
        "{}",
        dump(&buf)
    );
}

// Breaks if a video's extra `thumb:` line is not counted when deciding there is no room left
// (7 rows -> interior 4 rows = name, size, video, thumb).
#[test]
fn video_thumb_text_fills_the_interior_gives_none() {
    let (mut app, _dir) = focused_app("s13pi-small-video", |d| {
        write(d, "b.mp4");
        write(d, "b.jpg");
    });
    let (img, buf) = image_at(&mut app, 80, 7);
    assert_eq!(preview_row(&buf, "thumb: b.jpg"), 4, "{}", dump(&buf));
    assert!(img.is_none(), "got {img:?}; buffer:\n{}", dump(&buf));
}

// Breaks if the result is cached from an earlier frame instead of reflecting the last render
// (items focused -> Some, then focus returns to the module column -> None).
#[test]
fn module_column_focus_clears_the_image() {
    let (mut app, _dir) = focused_app("s13pi-unfocus", png);
    let (img, buf) = image_at(&mut app, 80, 24);
    assert!(
        img.is_some(),
        "focused must offer an image; buffer:\n{}",
        dump(&buf)
    );
    press(&mut app, 'h');
    let (img, buf) = image_at(&mut app, 80, 24);
    assert!(img.is_none(), "got {img:?}; buffer:\n{}", dump(&buf));
}

// Breaks if the Monitors module (no wallpapers) keeps offering the Theme image from a prior frame.
#[test]
fn monitors_module_has_no_image() {
    let (mut app, _dir) = focused_app("s13pi-monitors", png);
    assert!(image_at(&mut app, 80, 24).0.is_some());
    press(&mut app, 'h');
    press(&mut app, 'j'); // Monitors module
    press(&mut app, 'l');
    let (img, buf) = image_at(&mut app, 80, 24);
    assert!(img.is_none(), "got {img:?}; buffer:\n{}", dump(&buf));
}

// Breaks if an empty wallpaper dir yields an image (panic or bogus path).
#[test]
fn empty_wallpaper_dir_has_no_image() {
    let (mut app, _dir) = focused_app("s13pi-empty", |_| {});
    let (img, buf) = image_at(&mut app, 80, 24);
    assert!(img.is_none(), "got {img:?}; buffer:\n{}", dump(&buf));
}

// Breaks if a filter with zero matches keeps offering the previously hovered image.
#[test]
fn filter_with_no_matches_clears_the_image() {
    let (mut app, _dir) = focused_app("s13pi-nomatch", three_pngs);
    assert!(image_at(&mut app, 80, 24).0.is_some());
    press(&mut app, '/');
    type_str(&mut app, "zzz");
    enter(&mut app);
    let (img, buf) = image_at(&mut app, 80, 24);
    assert!(img.is_none(), "got {img:?}; buffer:\n{}", dump(&buf));
}

// Breaks if the image does not follow the cursor (stays on the first item).
#[test]
fn image_follows_the_cursor_down() {
    let (mut app, dir) = focused_app("s13pi-follow", three_pngs);
    assert_eq!(
        image_at(&mut app, 80, 24).0.map(|i| i.path),
        Some(dir.join("a1.png"))
    );
    press(&mut app, 'j');
    assert_eq!(
        image_at(&mut app, 80, 24).0.map(|i| i.path),
        Some(dir.join("b1.png"))
    );
}

// Breaks if jumping to the last row (`G`) does not move the image to the last item.
#[test]
fn capital_g_moves_the_image_to_the_last_item() {
    let (mut app, dir) = focused_app("s13pi-last", three_pngs);
    press(&mut app, 'G');
    assert_eq!(
        image_at(&mut app, 80, 24).0.map(|i| i.path),
        Some(dir.join("c1.png"))
    );
}

// Breaks if, under a filter, the visible row index is used on the unfiltered list: filter `b`
// leaves b1, b2 at filtered rows 0, 1 (unfiltered 1, 2); `j` must show b2, not b1.
#[test]
fn filtered_cursor_selects_the_highlighted_visible_row() {
    let (mut app, dir) = focused_app("s13pi-filter", |d| {
        for n in ["a1.png", "b1.png", "b2.png", "c1.png"] {
            write(d, n);
        }
    });
    press(&mut app, '/');
    type_str(&mut app, "b");
    enter(&mut app);
    assert_eq!(
        image_at(&mut app, 80, 24).0.map(|i| i.path),
        Some(dir.join("b1.png"))
    );
    press(&mut app, 'j');
    assert_eq!(
        image_at(&mut app, 80, 24).0.map(|i| i.path),
        Some(dir.join("b2.png"))
    );
}

//! S4.1-wallpaper-list-hardening: media types, case-insensitive natural order, subdirectory
//! grouping, and conservative thumbnail hiding in the Theme items column.
//!
//! At 80 columns the items column spans x in 20..40.

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

fn touch(path: &Path) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, b"x").unwrap();
}

/// Creates `files` in a fresh wallpaper dir, builds the App, presses `l`, renders 80x24.
fn render_with_files(tag: &str, files: &[&str]) -> Buffer {
    let home = tempdir(&format!("{tag}-home"));
    let dir = tempdir(&format!("{tag}-wall"));
    for f in files {
        touch(&dir.join(f));
    }
    let mut cfg = Config::default_for(&home);
    cfg.wallpaper_dir = dir;
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

fn items_row(buf: &Buffer, text: &str) -> u16 {
    items_hits(buf, text)
        .first()
        .unwrap_or_else(|| {
            panic!(
                "`{text}` not found in items column; buffer:\n{}",
                buffer_lines(buf).join("\n")
            )
        })
        .1
}

/// Asserts the given names appear in the items column in strictly increasing row order.
fn assert_order(buf: &Buffer, names: &[&str]) {
    let rows: Vec<u16> = names.iter().map(|n| items_row(buf, n)).collect();
    for i in 1..names.len() {
        assert!(
            rows[i - 1] < rows[i],
            "`{}` (row {}) must precede `{}` (row {}); buffer:\n{}",
            names[i - 1],
            rows[i - 1],
            names[i],
            rows[i],
            buffer_lines(buf).join("\n")
        );
    }
}

#[test]
fn media_types_and_case_insensitive_order() {
    // Catches: case-sensitive sort (Zebra/Mango before apple), missing webp/jpeg/gif/mkv or
    // upper-case extensions (.WEBM/.PNG), or non-media doc.pdf being listed.
    let buf = render_with_files(
        "s41-media",
        &[
            "Zebra.png",
            "apple.png",
            "Mango.PNG",
            "clip.WEBM",
            "pic.webp",
            "x.jpeg",
            "y.gif",
            "zoo.mkv",
            "doc.pdf",
        ],
    );
    assert_order(
        &buf,
        &[
            "apple.png",
            "clip.WEBM",
            "Mango.PNG",
            "pic.webp",
            "x.jpeg",
            "y.gif",
            "Zebra.png",
            "zoo.mkv",
        ],
    );
    assert!(
        items_hits(&buf, "doc.pdf").is_empty(),
        "doc.pdf must be hidden; buffer:\n{}",
        buffer_lines(&buf).join("\n")
    );
}

#[test]
fn lone_jpg_stays_when_no_video_shares_stem() {
    // Regression guard: a .jpg is hidden only when a video shares its stem; solo.jpg must
    // stay while b.jpg (thumbnail of b.mp4) is hidden.
    let buf = render_with_files("s41-solo", &["solo.jpg", "b.mp4", "b.jpg"]);
    assert_order(&buf, &["b.mp4", "solo.jpg"]);
    assert!(
        items_hits(&buf, "b.jpg").is_empty(),
        "b.jpg must be hidden; buffer:\n{}",
        buffer_lines(&buf).join("\n")
    );
}

#[test]
fn subdirectories_sorted_by_name_with_files_contiguous() {
    // Catches: sorting full paths as flat strings/natural-order (a1/x, a01/y, a1/z interleaving
    // the two directories) instead of grouping each subdirectory's files together.
    let buf = render_with_files("s41-subdirs", &["a1/x.png", "a01/y.png", "a1/z.png"]);
    assert_order(&buf, &["a01/y.png", "a1/x.png", "a1/z.png"]);
}

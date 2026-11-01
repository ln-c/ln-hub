//! Wallpaper discovery for the Theme items column.
//!
//! Lists images and videos in the wallpaper directory: top-level entries first, then entries
//! from one subdirectory level as `subdir/name`. Each group is sorted naturally. A `.jpg` that
//! shares its stem with a video in the same directory is that video's thumbnail and is hidden.

use std::cmp::Ordering;
use std::collections::HashSet;
use std::fs;
use std::iter::Peekable;
use std::path::{Path, PathBuf};
use std::str::Chars;

const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif"];
const VIDEO_EXTS: &[&str] = &["mp4", "webm", "mkv"];

/// Display names of the wallpapers in `dir`. A missing or unreadable directory yields none.
pub(crate) fn discover(dir: &Path) -> Vec<String> {
    let (files, subdirs) = read_entries(dir);
    let mut names = media_names(&files);

    let mut nested = Vec::new();
    for (sub, path) in &subdirs {
        let (sub_files, _) = read_entries(path);
        nested.extend(
            media_names(&sub_files)
                .into_iter()
                .map(|name| format!("{sub}/{name}")),
        );
    }
    nested.sort_by(|a, b| natural_cmp(a, b));

    names.extend(nested);
    names
}

/// Splits the entries of `dir` into file names and `(name, path)` subdirectories.
fn read_entries(dir: &Path) -> (Vec<String>, Vec<(String, PathBuf)>) {
    let mut files = Vec::new();
    let mut subdirs = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return (files, subdirs);
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            subdirs.push((name, path));
        } else if path.is_file() {
            files.push(name);
        }
    }
    (files, subdirs)
}

/// The wallpaper file names among `files` (one directory's listing), naturally sorted.
fn media_names(files: &[String]) -> Vec<String> {
    let video_stems: HashSet<String> = files
        .iter()
        .filter(|f| is_video(f))
        .map(|f| stem(f))
        .collect();
    let mut names: Vec<String> = files
        .iter()
        .filter(|f| is_wallpaper(f, &video_stems))
        .cloned()
        .collect();
    names.sort_by(|a, b| natural_cmp(a, b));
    names
}

/// A video, or an image that is not the thumbnail of a video in the same directory.
fn is_wallpaper(name: &str, video_stems: &HashSet<String>) -> bool {
    let ext = extension(name);
    if VIDEO_EXTS.contains(&ext.as_str()) {
        return true;
    }
    if !IMAGE_EXTS.contains(&ext.as_str()) {
        return false;
    }
    !(ext == "jpg" && video_stems.contains(&stem(name)))
}

fn is_video(name: &str) -> bool {
    VIDEO_EXTS.contains(&extension(name).as_str())
}

/// Lowercased extension, or an empty string when there is none.
fn extension(name: &str) -> String {
    Path::new(name)
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

fn stem(name: &str) -> String {
    Path::new(name)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Orders names so digit runs compare by value (`img2.png` before `img10.png`). Ties on the
/// natural order fall back to plain byte order, so the ordering is total.
fn natural_cmp(a: &str, b: &str) -> Ordering {
    natural_chunks(a, b).then_with(|| a.cmp(b))
}

fn natural_chunks(a: &str, b: &str) -> Ordering {
    let mut a = a.chars().peekable();
    let mut b = b.chars().peekable();
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let da = take_digits(&mut a);
                let db = take_digits(&mut b);
                // Compare the numbers as strings without leading zeros: no overflow, no panic.
                let (ta, tb) = (da.trim_start_matches('0'), db.trim_start_matches('0'));
                match ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb)) {
                    Ordering::Equal => {}
                    ord => return ord,
                }
            }
            (Some(x), Some(y)) => match x.cmp(&y) {
                Ordering::Equal => {
                    a.next();
                    b.next();
                }
                ord => return ord,
            },
        }
    }
}

fn take_digits(chars: &mut Peekable<Chars<'_>>) -> String {
    let mut digits = String::new();
    while let Some(c) = chars.next_if(char::is_ascii_digit) {
        digits.push(c);
    }
    digits
}

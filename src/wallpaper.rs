//! Wallpaper discovery and the applied-wallpaper file for the Theme items column.
//!
//! Lists images and videos in the wallpaper directory: top-level entries first, then entries
//! from one subdirectory level as `subdir/name`. Names sort naturally and case-insensitively;
//! subdirectories are taken in name order, each keeping its files together. A `.jpg` that
//! shares its stem with a video in the same directory is that video's thumbnail and is hidden.
//! Stems match case-sensitively; extensions match case-insensitively.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::fs;
use std::iter::Peekable;
use std::path::{Path, PathBuf};
use std::str::Chars;

const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif"];
const VIDEO_EXTS: &[&str] = &["mp4", "webm", "mkv"];

/// A wallpaper file, with the paths the other slices need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Wallpaper {
    /// Display name: `x.png`, or `sub/x.png` one subdirectory level down.
    pub(crate) name: String,
    /// `dir.join(name)`; absolute when `wallpaper_dir` is.
    pub(crate) path: PathBuf,
    pub(crate) kind: Kind,
    /// For a video, the same-directory `.jpg` that shares its stem (hidden from the list).
    pub(crate) thumb: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Image,
    Video,
}

/// A subdirectory of the wallpaper directory.
struct Subdir {
    name: String,
    path: PathBuf,
}

/// One file name, split once into the parts the listing compares.
struct Classified<'a> {
    name: &'a str,
    stem: &'a str,
    /// `None` for files that are not wallpapers, such as `doc.pdf`.
    kind: Option<Kind>,
    /// Whether the extension is `jpg`, the only image a video's thumbnail can be.
    jpg: bool,
}

/// The wallpapers in `dir`, in display order. A missing or unreadable directory yields none.
pub(crate) fn discover(dir: &Path) -> Vec<Wallpaper> {
    let (files, mut subdirs) = split_entries(dir);
    let mut wallpapers = list_media(dir, None, &files);

    subdirs.sort_by(|a, b| natural_cmp(&a.name, &b.name));
    for sub in &subdirs {
        let files = file_names(&sub.path);
        wallpapers.extend(list_media(&sub.path, Some(&sub.name), &files));
    }
    wallpapers
}

/// The path recorded in the applied-wallpaper file. Only the line terminator is stripped, so a
/// path's own trailing whitespace survives. A missing, unreadable, non-UTF-8, or empty file
/// yields `None`.
pub(crate) fn read_current(file: &Path) -> Option<PathBuf> {
    let content = fs::read_to_string(file).ok()?;
    let line = content.trim_end_matches(['\n', '\r']);
    (!line.is_empty()).then(|| PathBuf::from(line))
}

/// The wallpapers among `files`, the file names directly in `dir`, naturally sorted. `prefix` is
/// the subdirectory name for files one level down, shown as `prefix/name`.
fn list_media(dir: &Path, prefix: Option<&str>, files: &[String]) -> Vec<Wallpaper> {
    let mut names: Vec<&str> = files.iter().map(String::as_str).collect();
    names.sort_unstable();
    let classified: Vec<Classified> = names.into_iter().map(classify).collect();

    let video_stems: HashSet<&str> = classified
        .iter()
        .filter(|c| c.kind == Some(Kind::Video))
        .map(|c| c.stem)
        .collect();
    let is_thumb = |c: &Classified| c.jpg && video_stems.contains(c.stem);
    // The first `.jpg` (in byte order) per video stem is that video's thumbnail.
    let mut thumbs: HashMap<&str, &str> = HashMap::new();
    for c in classified.iter().filter(|c| is_thumb(c)) {
        thumbs.entry(c.stem).or_insert(c.name);
    }

    let mut wallpapers: Vec<Wallpaper> = classified
        .into_iter()
        .filter_map(|c| {
            let kind = c.kind?;
            if is_thumb(&c) {
                return None;
            }
            let thumb = match kind {
                Kind::Video => thumbs.get(c.stem).map(|t| dir.join(t)),
                Kind::Image => None,
            };
            Some(Wallpaper {
                name: match prefix {
                    Some(p) => format!("{p}/{}", c.name),
                    None => c.name.to_owned(),
                },
                path: dir.join(c.name),
                kind,
                thumb,
            })
        })
        .collect();
    wallpapers.sort_by(|a, b| natural_cmp(&a.name, &b.name));
    wallpapers
}

/// Splits one file name into its stem and extension, and classifies it by extension.
fn classify(name: &str) -> Classified<'_> {
    let path = Path::new(name);
    let stem = path.file_stem().and_then(OsStr::to_str).unwrap_or("");
    let ext = path.extension().and_then(OsStr::to_str).unwrap_or("");
    Classified {
        name,
        stem,
        kind: kind_of_ext(ext),
        jpg: ext.eq_ignore_ascii_case("jpg"),
    }
}

/// The kind of a file with extension `ext`, matched case-insensitively.
fn kind_of_ext(ext: &str) -> Option<Kind> {
    let in_list = |list: &[&str]| list.iter().any(|known| ext.eq_ignore_ascii_case(known));
    if in_list(VIDEO_EXTS) {
        Some(Kind::Video)
    } else if in_list(IMAGE_EXTS) {
        Some(Kind::Image)
    } else {
        None
    }
}

/// The files and subdirectories directly in `dir`.
fn split_entries(dir: &Path) -> (Vec<String>, Vec<Subdir>) {
    let mut files = Vec::new();
    let mut subdirs = Vec::new();
    for (name, path) in dir_entries(dir) {
        if path.is_dir() {
            subdirs.push(Subdir { name, path });
        } else if path.is_file() {
            files.push(name);
        }
    }
    (files, subdirs)
}

/// The files directly in `dir`. Only `is_file` is checked, so subdirectories are skipped
/// without the extra `is_dir` stat.
fn file_names(dir: &Path) -> Vec<String> {
    dir_entries(dir)
        .filter(|(_, path)| path.is_file())
        .map(|(name, _)| name)
        .collect()
}

/// The `(name, path)` of each entry in `dir`. A missing or unreadable directory yields none.
/// Names that are not valid UTF-8 are skipped.
fn dir_entries(dir: &Path) -> impl Iterator<Item = (String, PathBuf)> {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = entry.file_name().into_string().ok()?;
            Some((name, path))
        })
}

/// Orders names case-insensitively, with digit runs compared by value (`img2.png` before
/// `img10.png`). Ties on that order fall back to plain byte order, so the ordering is total.
fn natural_cmp(a: &str, b: &str) -> Ordering {
    natural_chunks(&a.to_lowercase(), &b.to_lowercase()).then_with(|| a.cmp(b))
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

//! S11-palette-live: UI colors come from the wallust palette (`cfg.colors_file`) and reload in
//! `App::tick` when `colors.sh` changes (by mtime).
//!
//! Rendered on a 120x24 buffer (as in S10): the module column is x 0..20 and the items column is
//! x 20..53. The highlight observable is the cell holding the first letter of the highlighted row
//! (`Theme` in the module column; `img1.png` in the Theme items). A palette highlight is bg=color4
//! with fg=the palette's `background`; the built-in fallback is today's style: REVERSED with fg and
//! bg `Color::Reset`.
//!
//! Only `System` is faked; `colors.sh` and the wallpaper dir are real temp files. Every mtime is
//! set explicitly with `set_modified`, so fs timestamp granularity cannot make a test flaky.

mod common;

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use common::*;
use ln_hub::{App, Config};
use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier};

const W: u16 = 120;
const H: u16 = 24;
const MODULES_X_END: u16 = 20;
const ITEMS_X_START: u16 = 20;
const ITEMS_X_END: u16 = 53;

const ROWS: [&str; 2] = ["img1.png", "img2.png"];

const AABBCC: Color = Color::Rgb(0xaa, 0xbb, 0xcc);
const C112233: Color = Color::Rgb(0x11, 0x22, 0x33);
const BG0D0E0F: Color = Color::Rgb(0x0d, 0x0e, 0x0f);
const BG305070: Color = Color::Rgb(0x30, 0x50, 0x70);

/// A fixed, arbitrary mtime base; `t(n)` is strictly newer for a larger `n`.
fn t(n: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000 + n * 10)
}

/// A full, valid wallust `colors.sh` whose `color4` value is `color4` (written verbatim), with
/// `skip` dropping the line of that key.
fn colors_sh(color4: &str, skip: Option<&str>) -> String {
    colors_sh_bg(color4, "#170000", skip)
}

/// Like `colors_sh`, with the `background` value also given (written verbatim).
fn colors_sh_bg(color4: &str, background: &str, skip: Option<&str>) -> String {
    let mut keys: Vec<(String, String)> = vec![
        ("background".into(), background.into()),
        ("foreground".into(), "#e0d0c0".into()),
    ];
    for i in 0..16 {
        let v = if i == 4 {
            color4.to_string()
        } else {
            format!("#0{:x}0{:x}0{:x}", i % 16, (i + 1) % 16, (i + 2) % 16)
        };
        keys.push((format!("color{i}"), v));
    }
    keys.iter()
        .filter(|(k, _)| Some(k.as_str()) != skip)
        .map(|(k, v)| format!("{k}='{v}'\n"))
        .collect()
}

/// Writes `content` to `path` and stamps it with mtime `t(n)`.
fn write_colors(path: &Path, content: &str, n: u64) {
    std::fs::write(path, content).unwrap();
    set_mtime(path, t(n));
}

/// A fresh colors.sh path (the file is NOT created).
fn colors_path(name: &str) -> PathBuf {
    tempdir(&format!("{name}-colors")).join("colors.sh")
}

/// App whose palette file is `colors`, over a real wallpaper dir with img1.png / img2.png.
fn app_with_colors(name: &str, colors: &Path) -> App {
    let home = tempdir(&format!("{name}-home"));
    let dir = tempdir(&format!("{name}-wall"));
    for n in ROWS {
        std::fs::write(dir.join(n), b"x").unwrap();
    }
    let mut cfg = Config::default_for(&home);
    cfg.wallpaper_dir = dir;
    cfg.colors_file = colors.to_path_buf();
    App::new(cfg, Box::new(FakeSystem::new()))
}

fn draw(app: &mut App) -> Buffer {
    draw_sized(app, W, H)
}

/// Style of the highlighted module row (`Theme`, selected at start).
fn module_style(app: &mut App) -> CellStyle {
    name_style(&draw(app), "Theme", 0, MODULES_X_END)
}

/// Style of the highlighted Theme item (`img1.png`); the items column must be focused.
fn item_style(app: &mut App) -> CellStyle {
    name_style(&draw(app), "img1.png", ITEMS_X_START, ITEMS_X_END)
}

/// Highlight bg of the module row and (after `l`) of the item row, in that order.
fn highlight_bgs(app: &mut App) -> (Color, Color) {
    let module = module_style(app).1;
    press(app, 'l');
    (module, item_style(app).1)
}

/// Highlight fg of the module row and (after `l`) of the item row, in that order.
fn highlight_fgs(app: &mut App) -> (Color, Color) {
    let module = module_style(app).0;
    press(app, 'l');
    (module, item_style(app).0)
}

/// Items-column x-range differs from s10's, so this wrapper stays local.
fn items_hit(buf: &Buffer, text: &str) -> Option<(u16, u16)> {
    items_hit_within(buf, text, ITEMS_X_START, ITEMS_X_END)
}

#[test]
fn palette_color4_is_the_highlight_bg_in_module_and_items_columns() {
    // Catches: the UI ignoring colors.sh (still plain REVERSED, bg Reset), or using a palette
    // slot other than color4 for the highlight, in either the module or the items column.
    let colors = colors_path("s11-initial");
    write_colors(&colors, &colors_sh("#aabbcc", None), 0);
    let mut a = app_with_colors("s11-initial", &colors);
    assert_eq!(module_style(&mut a).1, AABBCC, "module highlight bg");
    press(&mut a, 'l');
    assert_eq!(item_style(&mut a).1, AABBCC, "items highlight bg");
    let buf = draw(&mut a);
    assert_only_highlighted(&buf, &ROWS, "img1.png", items_hit);
}

#[test]
fn palette_background_is_the_highlight_fg_in_module_and_items_columns() {
    // Catches: a highlight that sets only bg=color4 and leaves fg at the terminal default (text
    // unreadable on light/dark color4), or an fg taken from `foreground` or another slot.
    let colors = colors_path("s11-fg");
    write_colors(&colors, &colors_sh_bg("#aabbcc", "#0d0e0f", None), 0);
    let mut a = app_with_colors("s11-fg", &colors);
    let module = module_style(&mut a);
    assert_eq!((module.0, module.1), (BG0D0E0F, AABBCC), "module fg/bg");
    press(&mut a, 'l');
    let item = item_style(&mut a);
    assert_eq!((item.0, item.1), (BG0D0E0F, AABBCC), "items fg/bg");
}

#[test]
fn tick_reload_updates_the_highlight_fg_to_the_new_background() {
    // Catches: a reload that refreshes bg (color4) but keeps the old fg (background), in either
    // column.
    let colors = colors_path("s11-fgreload");
    write_colors(&colors, &colors_sh_bg("#aabbcc", "#0d0e0f", None), 0);
    let mut a = app_with_colors("s11-fgreload", &colors);
    assert_eq!(highlight_fgs(&mut a), (BG0D0E0F, BG0D0E0F));

    write_colors(&colors, &colors_sh_bg("#aabbcc", "#305070", None), 1);
    a.tick();
    assert_eq!(item_style(&mut a).0, BG305070, "items highlight fg");
    assert_eq!(module_style(&mut a).0, BG305070, "module highlight fg");
    assert_eq!(item_style(&mut a).1, AABBCC, "bg unchanged");
}

#[test]
fn tick_reloads_a_newer_colors_file_with_no_job_in_flight() {
    // Catches: tick returning early when there is no job (never reloading), or a reload that
    // updates only one column.
    let colors = colors_path("s11-reload");
    write_colors(&colors, &colors_sh("#aabbcc", None), 0);
    let mut a = app_with_colors("s11-reload", &colors);
    assert_eq!(module_style(&mut a).1, AABBCC);
    press(&mut a, 'l');
    assert_eq!(item_style(&mut a).1, AABBCC);

    write_colors(&colors, &colors_sh("#112233", None), 1);
    a.tick();
    assert_eq!(item_style(&mut a).1, C112233, "items highlight bg");
    assert_eq!(module_style(&mut a).1, C112233, "module highlight bg");
}

#[test]
fn render_alone_does_not_reload_the_palette() {
    // Catches: the reload happening in render (disk I/O per frame) instead of in tick.
    let colors = colors_path("s11-render");
    write_colors(&colors, &colors_sh("#aabbcc", None), 0);
    let mut a = app_with_colors("s11-render", &colors);
    assert_eq!(module_style(&mut a).1, AABBCC);

    write_colors(&colors, &colors_sh("#112233", None), 1);
    assert_eq!(module_style(&mut a).1, AABBCC, "render must not reread");
    assert_eq!(
        module_style(&mut a).1,
        AABBCC,
        "render must not reread (2nd frame)"
    );
    a.tick();
    assert_eq!(module_style(&mut a).1, C112233, "after tick");
}

#[test]
fn unchanged_mtime_is_not_reread() {
    // Catches: rereading the file on every tick (or comparing content/size) instead of skipping
    // when the mtime is unchanged.
    let colors = colors_path("s11-same");
    write_colors(&colors, &colors_sh("#aabbcc", None), 0);
    let mut a = app_with_colors("s11-same", &colors);
    assert_eq!(module_style(&mut a).1, AABBCC);

    write_colors(&colors, &colors_sh("#112233", None), 0);
    a.tick();
    a.tick();
    assert_eq!(module_style(&mut a).1, AABBCC);
}

#[test]
fn a_second_rewrite_with_the_already_seen_mtime_is_ignored_after_a_reload() {
    // Catches: remembering only the mtime of the FIRST load (so every tick after a reload
    // rereads), or not updating the remembered mtime when a reload succeeds.
    let colors = colors_path("s11-twice");
    write_colors(&colors, &colors_sh("#aabbcc", None), 1);
    let mut a = app_with_colors("s11-twice", &colors);
    write_colors(&colors, &colors_sh("#112233", None), 2);
    a.tick();
    assert_eq!(module_style(&mut a).1, C112233);
    // Same mtime again with other content: still the already-loaded palette.
    write_colors(&colors, &colors_sh("#aabbcc", None), 2);
    a.tick();
    assert_eq!(module_style(&mut a).1, C112233);
}

fn assert_fallback(app: &mut App) {
    let (fg, bg, m) = module_style(app);
    assert_eq!(fg, Color::Reset, "module fallback fg");
    assert_eq!(bg, Color::Reset, "module fallback bg");
    assert!(
        m.contains(Modifier::REVERSED),
        "module fallback must be REVERSED, got {m:?}"
    );
    press(app, 'l');
    let (fg, bg, m) = item_style(app);
    assert_eq!(fg, Color::Reset, "items fallback fg");
    assert_eq!(bg, Color::Reset, "items fallback bg");
    assert!(
        m.contains(Modifier::REVERSED),
        "items fallback must be REVERSED, got {m:?}"
    );
    let buf = draw(app);
    assert_only_highlighted(&buf, &ROWS, "img1.png", items_hit);
}

#[test]
fn missing_colors_file_uses_the_builtin_fallback() {
    // Catches: panicking on a missing colors.sh, or a fallback that is not today's
    // terminal-default REVERSED highlight.
    let colors = colors_path("s11-missing");
    let mut a = app_with_colors("s11-missing", &colors);
    a.tick();
    assert_fallback(&mut a);
}

#[test]
fn invalid_color_value_at_start_uses_the_builtin_fallback() {
    // Catches: panicking on (or half-applying) an unparsable color value like `color4='zzz'`.
    let colors = colors_path("s11-badval");
    write_colors(&colors, &colors_sh("zzz", None), 0);
    let mut a = app_with_colors("s11-badval", &colors);
    a.tick();
    assert_fallback(&mut a);
}

#[test]
fn missing_key_at_start_uses_the_builtin_fallback() {
    // Catches: using a partial palette (or panicking) when a required key such as color15 is absent.
    let colors = colors_path("s11-nokey");
    write_colors(&colors, &colors_sh("#aabbcc", Some("color15")), 0);
    let mut a = app_with_colors("s11-nokey", &colors);
    a.tick();
    assert_fallback(&mut a);
}

#[test]
fn invalid_rewrite_keeps_the_last_good_palette_and_a_valid_one_recovers() {
    // Catches: dropping to the fallback (or panicking) when wallust is mid-write and the new file
    // is invalid, and a reload that gets stuck after an invalid file instead of recovering.
    let colors = colors_path("s11-keep");
    write_colors(&colors, &colors_sh("#aabbcc", None), 0);
    let mut a = app_with_colors("s11-keep", &colors);
    assert_eq!(module_style(&mut a).1, AABBCC);

    write_colors(&colors, &colors_sh("zzz", None), 1);
    a.tick();
    assert_eq!(
        module_style(&mut a).1,
        AABBCC,
        "invalid value keeps last good"
    );

    write_colors(&colors, &colors_sh("#aabbcc", Some("background")), 2);
    a.tick();
    assert_eq!(
        module_style(&mut a).1,
        AABBCC,
        "missing key keeps last good"
    );

    write_colors(&colors, "", 3);
    a.tick();
    assert_eq!(module_style(&mut a).1, AABBCC, "empty file keeps last good");

    write_colors(&colors, &colors_sh("#112233", None), 4);
    a.tick();
    assert_eq!(module_style(&mut a).1, C112233, "recovers on a valid file");
}

#[test]
fn deleted_colors_file_keeps_the_last_good_palette() {
    // Catches: panicking, or reverting to the fallback, when colors.sh disappears after a good load.
    let colors = colors_path("s11-deleted");
    write_colors(&colors, &colors_sh("#aabbcc", None), 0);
    let mut a = app_with_colors("s11-deleted", &colors);
    assert_eq!(module_style(&mut a).1, AABBCC);

    std::fs::remove_file(&colors).unwrap();
    a.tick();
    assert_eq!(module_style(&mut a).1, AABBCC);
    press(&mut a, 'l');
    assert_eq!(item_style(&mut a).1, AABBCC);
}

#[test]
fn colors_file_that_appears_later_is_picked_up_by_tick() {
    // Catches: treating a missing file at start as permanent (never retrying), so wallust's first
    // write after the hub started is ignored.
    let colors = colors_path("s11-appears");
    let mut a = app_with_colors("s11-appears", &colors);
    a.tick();
    assert_eq!(module_style(&mut a).1, Color::Reset);

    write_colors(&colors, &colors_sh("#aabbcc", None), 0);
    a.tick();
    assert_eq!(highlight_bgs(&mut a), (AABBCC, AABBCC));
}

#[test]
fn valid_file_replacing_an_invalid_one_at_start_is_picked_up_by_tick() {
    // Catches: remembering the invalid file's mtime as "handled" in a way that blocks a later
    // valid file, or never leaving the fallback once started on it.
    let colors = colors_path("s11-badthengood");
    write_colors(&colors, &colors_sh("zzz", None), 0);
    let mut a = app_with_colors("s11-badthengood", &colors);
    a.tick();
    assert_eq!(module_style(&mut a).1, Color::Reset);

    write_colors(&colors, &colors_sh("#112233", None), 1);
    a.tick();
    assert_eq!(module_style(&mut a).1, C112233);
}

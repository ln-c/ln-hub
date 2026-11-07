//! S12a-keymap: `h`/`k` and the arrow keys in the module and items columns, sticky focus, and
//! key-conflict validation of `[keymap]`.
//!
//! Rendered on a 120x24 buffer (as in S9/S10): the module column is x 0..20 and the items column
//! interior is x 21..53. "Shown" means fully inside the items column, so module labels and the
//! preview never count. The wallpaper dir (real temp dir) holds img1.png / img2.png; the fake
//! `hyprctl` lists two enabled monitors. Only `System` is faked.

mod common;

use std::path::Path;

use common::*;
use ln_hub::{App, Config, Module};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;

const W: u16 = 120;
const H: u16 = 24;
const ITEMS_X_START: u16 = 21;
const ITEMS_X_END: u16 = 53;

const EDP: &str = "eDP-1  2880x1800@120  on";
const DP: &str = "DP-1  2560x1440@60  on";
const HDMI: &str = "HDMI-A-1  1920x1080@60  on";
const MONITOR_ROWS: [&str; 3] = [EDP, DP, HDMI];
// Three rows each: with two, "only row X is highlighted" cannot tell X from the other row.
const WALLPAPERS: [&str; 3] = ["img1.png", "img2.png", "img3.png"];

const MONITORS_JSON: &str = r#"[
  {"name": "eDP-1", "width": 2880, "height": 1800, "refreshRate": 120.000, "disabled": false},
  {"name": "DP-1", "width": 2560, "height": 1440, "refreshRate": 60.000, "disabled": false},
  {"name": "HDMI-A-1", "width": 1920, "height": 1080, "refreshRate": 60.000, "disabled": false}
]"#;

fn sys() -> FakeSystem {
    let sys = FakeSystem::new();
    sys.set_stdout("hyprctl", MONITORS_JSON);
    sys
}

/// App over a real temp wallpaper dir with img1..img3.png, with `keys` pressed in order.
fn build(name: &str, modules: Option<Vec<Module>>, keys: &[char]) -> App {
    let home = tempdir(&format!("{name}-home"));
    let dir = tempdir(&format!("{name}-wall"));
    for n in WALLPAPERS {
        std::fs::write(dir.join(n), b"x").unwrap();
    }
    let mut cfg = Config::default_for(&home);
    cfg.wallpaper_dir = dir;
    if let Some(m) = modules {
        cfg.modules = m;
    }
    let mut app = App::new(cfg, Box::new(sys()));
    for &c in keys {
        press(&mut app, c);
    }
    app
}

/// Default module order: Theme, Monitors.
fn app(name: &str, keys: &[char]) -> App {
    build(name, None, keys)
}

/// Monitors listed before Theme.
fn app_monitors_first(name: &str, keys: &[char]) -> App {
    build(name, Some(vec![Module::Monitors, Module::Theme]), keys)
}

fn draw(app: &mut App) -> Buffer {
    draw_sized(app, W, H)
}

fn hit(buf: &Buffer, text: &str) -> Option<(u16, u16)> {
    items_hit_within(buf, text, ITEMS_X_START, ITEMS_X_END)
}

/// What the items column currently shows: "wallpapers", "monitors" or "blank". Panics on a mix.
fn items_view(app: &mut App) -> &'static str {
    let buf = draw(app);
    let wall = WALLPAPERS.iter().any(|t| hit(&buf, t).is_some());
    let mon = MONITOR_ROWS.iter().any(|t| hit(&buf, t).is_some());
    match (wall, mon) {
        (true, false) => "wallpapers",
        (false, true) => "monitors",
        (false, false) => "blank",
        (true, true) => panic!("both lists shown; buffer:\n{}", dump(&buf)),
    }
}

fn assert_view(app: &mut App, expected: &str) {
    let got = items_view(app);
    assert_eq!(
        got,
        expected,
        "items column shows {got}, expected {expected}; buffer:\n{}",
        dump(&draw(app))
    );
}

fn assert_highlighted_wallpaper(app: &mut App, expected: &str) {
    let buf = draw(app);
    assert_only_highlighted(&buf, &WALLPAPERS, expected, hit);
}

fn assert_highlighted_monitor(app: &mut App, expected: &str) {
    let buf = draw(app);
    assert_only_highlighted(&buf, &MONITOR_ROWS, expected, hit);
}

fn arrow(app: &mut App, code: KeyCode) {
    app.handle_key(key(code));
}

// ---- module column: k and arrows ---------------------------------------------------------------

#[test]
fn k_moves_the_module_selection_up_and_clamps_at_the_top() {
    // Catches: no `k` in the module column (selection stuck on Monitors), or `k` wrapping from
    // the first module to the last. `l` reveals which module is selected.
    let mut a = app("s12k-up", &['j', 'k']);
    arrow(&mut a, KeyCode::Char('l'));
    assert_view(&mut a, "wallpapers");

    let mut a = app("s12k-clamp", &['j', 'k', 'k']);
    arrow(&mut a, KeyCode::Char('l'));
    assert_view(&mut a, "wallpapers");
}

#[test]
fn k_on_the_first_module_does_nothing() {
    // GUARD (passes already): `k` on the first module neither wraps to the last one nor focuses
    // the items. With Monitors first, a wrapping `k` would select Theme.
    let mut a = app_monitors_first("s12k-first", &['k']);
    assert_view(&mut a, "blank");
    press(&mut a, 'l');
    assert_view(&mut a, "monitors");
}

#[test]
fn arrow_down_and_up_move_the_module_selection_like_j_and_k() {
    // Catches: arrow keys ignored in the module column. Down must select Monitors, a second
    // Down must clamp, Up must select Theme again, and a second Up must clamp.
    let mut a = app("s12k-arrows-mod", &[]);
    arrow(&mut a, KeyCode::Down);
    arrow(&mut a, KeyCode::Down);
    press(&mut a, 'l');
    assert_view(&mut a, "monitors");

    let mut a = app("s12k-arrows-mod2", &[]);
    arrow(&mut a, KeyCode::Down);
    arrow(&mut a, KeyCode::Up);
    arrow(&mut a, KeyCode::Up);
    press(&mut a, 'l');
    assert_view(&mut a, "wallpapers");
}

// ---- h / Left and Right ------------------------------------------------------------------------

#[test]
fn h_in_the_items_column_returns_focus_to_the_module_column() {
    // Catches: `h` unbound (items stay focused), or `h` also changing the module selection.
    let mut a = app("s12h-back", &[]);
    assert_view(&mut a, "blank");
    press(&mut a, 'l');
    assert_view(&mut a, "wallpapers");
    press(&mut a, 'h');
    assert_view(&mut a, "blank");
    // The selection is unchanged: `l` brings the same (Theme) items back.
    press(&mut a, 'l');
    assert_view(&mut a, "wallpapers");
}

#[test]
fn after_h_j_and_k_move_the_module_selection_not_the_items_cursor() {
    // Catches: focus staying on the items after `h` (j moves the wallpaper cursor instead of
    // selecting Monitors), or the module selection not being movable after `h`.
    let mut a = app("s12h-jk", &['l', 'h']);
    press(&mut a, 'j');
    assert_view(&mut a, "blank");
    press(&mut a, 'l');
    assert_view(&mut a, "monitors");
    press(&mut a, 'h');
    press(&mut a, 'k');
    press(&mut a, 'l');
    assert_view(&mut a, "wallpapers");
    // The wallpaper cursor was never moved by those j/k presses.
    assert_highlighted_wallpaper(&mut a, "img1.png");
}

#[test]
fn h_in_the_monitors_items_returns_focus_and_keeps_the_monitors_selected() {
    // Catches: `h` doing nothing in the Monitors items, or the Monitors module being dropped
    // (selection reset to the first module, here Theme-less Monitors-first order).
    let mut a = app_monitors_first("s12h-mon", &['l', 'j']);
    assert_highlighted_monitor(&mut a, DP);
    press(&mut a, 'h');
    assert_view(&mut a, "blank");
    press(&mut a, 'j');
    press(&mut a, 'l');
    assert_view(&mut a, "wallpapers");
}

#[test]
fn h_in_the_module_column_does_nothing() {
    // GUARD (passes already): `h` in the module column must neither move the selection (a
    // buggy `h` as "up" would select Theme) nor focus the items.
    let mut a = app("s12h-noop", &['j', 'h']);
    assert_view(&mut a, "blank");
    press(&mut a, 'l');
    assert_view(&mut a, "monitors");
}

#[test]
fn right_and_left_arrows_move_focus_like_l_and_h() {
    // Catches: arrows unbound for focus changes.
    let mut a = app("s12h-arrows", &[]);
    arrow(&mut a, KeyCode::Right);
    assert_view(&mut a, "wallpapers");
    arrow(&mut a, KeyCode::Left);
    assert_view(&mut a, "blank");
    arrow(&mut a, KeyCode::Left);
    assert_view(&mut a, "blank");
    arrow(&mut a, KeyCode::Right);
    assert_view(&mut a, "wallpapers");
}

// ---- arrows inside the items column ------------------------------------------------------------

#[test]
fn arrow_down_and_up_move_the_wallpaper_cursor() {
    // Catches: arrow keys ignored in the Theme items column.
    let mut a = app("s12a-theme", &['l']);
    assert_highlighted_wallpaper(&mut a, "img1.png");
    arrow(&mut a, KeyCode::Down);
    assert_highlighted_wallpaper(&mut a, "img2.png");
    arrow(&mut a, KeyCode::Down);
    assert_highlighted_wallpaper(&mut a, "img3.png");
    arrow(&mut a, KeyCode::Up);
    assert_highlighted_wallpaper(&mut a, "img2.png");
}

#[test]
fn arrow_down_and_up_move_the_monitor_cursor() {
    // Catches: arrow keys ignored in the Monitors items column.
    let mut a = app("s12a-mon", &['j', 'l']);
    assert_highlighted_monitor(&mut a, EDP);
    arrow(&mut a, KeyCode::Down);
    assert_highlighted_monitor(&mut a, DP);
    arrow(&mut a, KeyCode::Down);
    assert_highlighted_monitor(&mut a, HDMI);
    arrow(&mut a, KeyCode::Up);
    assert_highlighted_monitor(&mut a, DP);
}

// ---- sticky focus ------------------------------------------------------------------------------

#[test]
fn j_in_the_monitors_items_keeps_the_monitors_view_when_monitors_come_first() {
    // CHARACTERISATION (S9 note; passes already): with modules = [Monitors, Theme], `l` then `j`
    // moves the monitor cursor; focus must not fall through to the module column, which would
    // select Theme and swap the view to wallpapers.
    let mut a = app_monitors_first("s12-sticky", &['l']);
    assert_highlighted_monitor(&mut a, EDP);
    press(&mut a, 'j');
    assert_view(&mut a, "monitors");
    assert_highlighted_monitor(&mut a, DP);
}

// ---- key-conflict validation -------------------------------------------------------------------

fn quit_config(key: &str) -> Result<Config, String> {
    Config::from_toml(&format!("[keymap]\nquit = \"{key}\""), Path::new("/h"))
        .map_err(|e| e.to_string())
}

#[test]
fn quit_may_not_take_a_built_in_key() {
    // Catches: no conflict validation (`quit = "j"` accepted, shadowing `j`), a message that
    // omits the offending key, or only some built-ins (h j k l f s /) being protected. The key
    // is checked quoted (`"j"`): bare `f`, `k`, `l` also occur in the words "key conflict".
    for k in ["h", "j", "k", "l", "f", "s", "/"] {
        match quit_config(k) {
            Ok(_) => panic!("quit = {k:?} must be rejected as a key conflict"),
            Err(msg) => {
                assert!(
                    msg.contains("key conflict"),
                    "quit = {k:?}: message must contain `key conflict`, got {msg:?}"
                );
                assert!(
                    msg.contains(&format!("\"{k}\"")),
                    "quit = {k:?}: message must name the key as \"{k}\", got {msg:?}"
                );
            }
        }
    }
}

#[test]
fn quit_on_a_free_key_is_accepted_and_works() {
    // GUARD (passes already): the conflict check must not reject free keys, and the remapped
    // quit key must still quit while the built-ins keep their meaning.
    let cfg = quit_config("x").expect("quit = \"x\" is free and must be accepted");
    let mut a = App::new(cfg, Box::new(FakeSystem::new()));
    press(&mut a, 'j');
    assert!(!a.should_quit(), "`j` must not quit");
    press(&mut a, 'x');
    assert!(a.should_quit(), "`x` must quit");
}

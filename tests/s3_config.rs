//! S3-config: `config.toml` overrides default paths, module order/visibility and keys.
//!
//! Module column observable: text found at x < 80/3 (the leftmost column), as in S1.

mod common;

use common::*;
use ln_hub::{App, Config, ConfigError, Module};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use std::path::{Path, PathBuf};

const W: u16 = 80;
const H: u16 = 24;
const LEFT_COL_MAX_X: u16 = W / 3;

fn home() -> PathBuf {
    PathBuf::from("/h")
}

fn parse(toml: &str) -> Config {
    match Config::from_toml(toml, Path::new("/h")) {
        Ok(c) => c,
        Err(_) => panic!("from_toml rejected valid config: {toml:?}"),
    }
}

fn app_from(toml: &str) -> App {
    App::new(parse(toml), Box::new(FakeSystem::new()))
}

fn draw(app: &mut App) -> Buffer {
    let mut term = Terminal::new(TestBackend::new(W, H)).unwrap();
    term.draw(|f| app.render(f)).unwrap();
    term.backend().buffer().clone()
}

fn left_rows(buf: &Buffer, label: &str) -> Vec<u16> {
    find_text(buf, label)
        .into_iter()
        .filter(|&(x, _)| x < LEFT_COL_MAX_X)
        .map(|(_, y)| y)
        .collect()
}

#[test]
fn default_for_resolves_paths_under_home() {
    // REGRESSION GUARD (passes already): default_for must keep resolving every path under
    // `home`. The "App lists wallpapers from it" part of the spec is S4; here we assert the
    // resolved Config paths. Catches a wrong default path or module list.
    let cfg = Config::default_for(Path::new("/h"));
    assert_eq!(cfg.wallpaper_dir, PathBuf::from("/h/Pictures/Wallpapers"));
    assert_eq!(
        cfg.current_wallpaper_file,
        PathBuf::from("/h/.cache/current_wallpaper")
    );
    assert_eq!(
        cfg.colors_file,
        PathBuf::from("/h/.cache/wallust/colors.sh")
    );
    assert_eq!(cfg.hypr_env_file, PathBuf::from("/h/.config/hypr/env.lua"));
    assert_eq!(cfg.modules, vec![Module::Theme, Module::Monitors]);
}

#[test]
fn modules_override_hides_unlisted_module() {
    // Catches: `modules` key ignored (Theme still rendered) or Monitors not rendered.
    let mut app = app_from(r#"modules = ["Monitors"]"#);
    let buf = draw(&mut app);
    assert!(
        !left_rows(&buf, "Monitors").is_empty(),
        "Monitors must be in the module column; buffer:\n{}",
        buffer_lines(&buf).join("\n")
    );
    assert!(
        left_rows(&buf, "Theme").is_empty(),
        "Theme must not be in the module column; buffer:\n{}",
        buffer_lines(&buf).join("\n")
    );
}

#[test]
fn modules_override_sets_order() {
    // Catches: configured module order ignored (default Theme-then-Monitors kept).
    let mut app = app_from(r#"modules = ["Monitors", "Theme"]"#);
    let buf = draw(&mut app);
    let mon = left_rows(&buf, "Monitors");
    let theme = left_rows(&buf, "Theme");
    assert!(
        !mon.is_empty() && !theme.is_empty(),
        "both modules must be rendered; buffer:\n{}",
        buffer_lines(&buf).join("\n")
    );
    assert!(
        mon[0] < theme[0],
        "Monitors (row {}) must be above Theme (row {})",
        mon[0],
        theme[0]
    );
}

#[test]
fn keymap_quit_override_replaces_default_key() {
    // Catches: keymap override ignored (`q` still quits) or new key not bound (`x` no-op).
    let mut app = app_from("[keymap]\nquit = \"x\"");
    app.handle_key(key(KeyCode::Char('q')));
    assert!(!app.should_quit(), "`q` must no longer quit");
    app.handle_key(key(KeyCode::Char('x')));
    assert!(app.should_quit(), "`x` must quit");
}

#[test]
fn unknown_module_is_an_error() {
    // Catches: unknown module names silently dropped/accepted instead of UnknownModule.
    match Config::from_toml(r#"modules = ["Audio"]"#, &home()) {
        Err(ConfigError::UnknownModule(ref m)) if m.to_string() == "Audio" => {}
        Err(_) => panic!("expected ConfigError::UnknownModule(\"Audio\"), got another error"),
        Ok(_) => panic!("expected ConfigError::UnknownModule(\"Audio\"), got Ok"),
    }
}

#[test]
fn tilde_expands_to_home_and_other_fields_keep_defaults() {
    // Catches: `~` left unexpanded, or overlay clobbering the other defaults.
    let cfg = parse(r#"wallpaper_dir = "~/walls""#);
    assert_eq!(cfg.wallpaper_dir, PathBuf::from("/h/walls"));
    assert_eq!(
        cfg.colors_file,
        PathBuf::from("/h/.cache/wallust/colors.sh")
    );
    assert_eq!(cfg.hypr_env_file, PathBuf::from("/h/.config/hypr/env.lua"));
}

#[test]
fn empty_toml_equals_defaults() {
    // Catches: from_toml("") failing or producing something other than default_for(home).
    assert_eq!(parse(""), Config::default_for(Path::new("/h")));
}

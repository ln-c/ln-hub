//! S3.1-config-hardening: `Config::from_toml` rejects configs that would break `App` or hide typos.

use ln_hub::{Config, ConfigError};
use std::path::{Path, PathBuf};

fn parse(toml: &str) -> Result<Config, ConfigError> {
    Config::from_toml(toml, Path::new("/h"))
}

#[test]
fn double_slash_after_tilde_stays_under_home() {
    // Catches `~//walls` expanding to `/walls` (absolute path replaces home in `join`).
    let cfg = match parse("wallpaper_dir = \"~//walls\"") {
        Ok(c) => c,
        Err(e) => panic!("from_toml rejected valid config: {e:?}"),
    };
    assert_eq!(cfg.wallpaper_dir, PathBuf::from("/h/walls"));
}

#[test]
fn empty_modules_list_is_rejected() {
    // Catches accepting `modules = []`, which leaves the App with nothing to select.
    match parse("modules = []") {
        Err(ConfigError::NoModules) => {}
        other => panic!("expected Err(NoModules), got {other:?}"),
    }
}

#[test]
fn duplicate_module_is_rejected_with_its_name() {
    // Catches accepting the same module twice (or reporting the wrong name).
    match parse("modules = [\"Theme\", \"Theme\"]") {
        Err(ConfigError::DuplicateModule(name)) => assert_eq!(&*name, "Theme"),
        other => panic!("expected Err(DuplicateModule(\"Theme\")), got {other:?}"),
    }
}

#[test]
fn unknown_top_level_key_is_a_parse_error_naming_the_key() {
    // Catches silently ignoring a typo'd top-level key (serde without deny_unknown_fields).
    match parse("wallpaper_dr = \"x\"") {
        Err(ConfigError::Parse(msg)) => assert!(
            msg.contains("wallpaper_dr"),
            "message should name the key, got: {msg:?}"
        ),
        other => panic!("expected Err(Parse(..)), got {other:?}"),
    }
}

#[test]
fn unknown_keymap_key_is_a_parse_error_naming_the_key() {
    // Catches silently ignoring a typo'd `[keymap]` key such as `quite`.
    match parse("[keymap]\nquite = \"x\"") {
        Err(ConfigError::Parse(msg)) => assert!(
            msg.contains("quite"),
            "message should name the key, got: {msg:?}"
        ),
        other => panic!("expected Err(Parse(..)), got {other:?}"),
    }
}

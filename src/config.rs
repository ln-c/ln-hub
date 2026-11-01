use std::fmt;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// A module shown in the module column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Module {
    Theme,
    Monitors,
}

impl Module {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Module::Theme => "Theme",
            Module::Monitors => "Monitors",
        }
    }

    fn from_name(name: &str) -> Result<Module, ConfigError> {
        match name {
            "Theme" => Ok(Module::Theme),
            "Monitors" => Ok(Module::Monitors),
            other => Err(ConfigError::UnknownModule(other.into())),
        }
    }
}

/// Key bindings. Only `quit` is configurable so far.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keymap {
    pub(crate) quit: char,
}

/// Errors from [`Config::from_toml`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// `modules` names a module that does not exist.
    UnknownModule(Box<str>),
    /// `modules` is present but empty.
    NoModules,
    /// `modules` names the same module more than once.
    DuplicateModule(Box<str>),
    /// A `[keymap]` value is not a single character.
    InvalidKey(String),
    /// The TOML text could not be parsed or has values of the wrong type.
    Parse(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::UnknownModule(name) => write!(f, "unknown module: {name}"),
            ConfigError::NoModules => write!(f, "modules must not be empty"),
            ConfigError::DuplicateModule(name) => write!(f, "duplicate module: {name}"),
            ConfigError::InvalidKey(key) => {
                write!(f, "invalid key: {key:?} (expected one character)")
            }
            ConfigError::Parse(msg) => write!(f, "invalid config: {msg}"),
        }
    }
}

impl std::error::Error for ConfigError {}

/// Resolved paths, module list and keymap for the hub.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub wallpaper_dir: PathBuf,
    pub current_wallpaper_file: PathBuf,
    pub colors_file: PathBuf,
    pub hypr_env_file: PathBuf,
    pub modules: Vec<Module>,
    pub(crate) keymap: Keymap,
}

/// On-disk shape of `config.toml`; every key is optional.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    wallpaper_dir: Option<String>,
    current_wallpaper_file: Option<String>,
    colors_file: Option<String>,
    hypr_env_file: Option<String>,
    modules: Option<Vec<String>>,
    keymap: Option<RawKeymap>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawKeymap {
    quit: Option<String>,
}

impl Config {
    /// Default configuration rooted at `home`.
    pub fn default_for(home: &Path) -> Config {
        Config {
            wallpaper_dir: home.join("Pictures/Wallpapers"),
            current_wallpaper_file: home.join(".cache/current_wallpaper"),
            colors_file: home.join(".cache/wallust/colors.sh"),
            hypr_env_file: home.join(".config/hypr/env.lua"),
            modules: vec![Module::Theme, Module::Monitors],
            keymap: Keymap { quit: 'q' },
        }
    }

    /// Parses `config.toml` text, overlaying it on [`Config::default_for`]`(home)`.
    /// A leading `~` in path values expands to `home`.
    pub fn from_toml(s: &str, home: &Path) -> Result<Config, ConfigError> {
        let raw: RawConfig = toml::from_str(s).map_err(|e| ConfigError::Parse(e.to_string()))?;
        let mut cfg = Config::default_for(home);

        if let Some(p) = raw.wallpaper_dir {
            cfg.wallpaper_dir = expand_home(&p, home);
        }
        if let Some(p) = raw.current_wallpaper_file {
            cfg.current_wallpaper_file = expand_home(&p, home);
        }
        if let Some(p) = raw.colors_file {
            cfg.colors_file = expand_home(&p, home);
        }
        if let Some(p) = raw.hypr_env_file {
            cfg.hypr_env_file = expand_home(&p, home);
        }
        if let Some(names) = raw.modules {
            if names.is_empty() {
                return Err(ConfigError::NoModules);
            }
            let mut modules: Vec<Module> = Vec::with_capacity(names.len());
            for name in &names {
                let module = Module::from_name(name)?;
                if modules.contains(&module) {
                    return Err(ConfigError::DuplicateModule(name.as_str().into()));
                }
                modules.push(module);
            }
            cfg.modules = modules;
        }
        if let Some(quit) = raw.keymap.and_then(|k| k.quit) {
            cfg.keymap.quit = single_char(&quit)?;
        }
        Ok(cfg)
    }
}

fn expand_home(value: &str, home: &Path) -> PathBuf {
    match value.strip_prefix('~') {
        Some("") => home.to_path_buf(),
        Some(rest) if rest.starts_with('/') => home.join(rest.trim_start_matches('/')),
        _ => PathBuf::from(value),
    }
}

fn single_char(key: &str) -> Result<char, ConfigError> {
    let mut chars = key.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => Ok(c),
        _ => Err(ConfigError::InvalidKey(key.to_string())),
    }
}

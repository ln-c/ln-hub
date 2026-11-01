use std::path::{Path, PathBuf};

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
}

/// Resolved paths and module list for the hub.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub wallpaper_dir: PathBuf,
    pub current_wallpaper_file: PathBuf,
    pub colors_file: PathBuf,
    pub hypr_env_file: PathBuf,
    pub modules: Vec<Module>,
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
        }
    }
}

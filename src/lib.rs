//! ln-hub: a yazi-style TUI hub for the ln-* scripts.

mod app;
mod config;
mod palette;
mod system;

pub use app::App;
pub use config::{Config, ConfigError, Module};
pub use palette::{Palette, PaletteError};
pub use system::{JobId, JobState, System};

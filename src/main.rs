use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use ln_hub::{App, Config, ProcSystem};
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyEventKind};

fn run(terminal: &mut DefaultTerminal, app: &mut App) -> io::Result<()> {
    while !app.should_quit() {
        terminal.draw(|f| app.render(f))?;
        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            app.handle_key(key);
        }
    }
    Ok(())
}

const USAGE: &str = "usage: ln-hub [--version|--config <path>]";

enum Cli {
    Version,
    Run { config: Option<PathBuf> },
}

fn parse_args(args: impl Iterator<Item = String>) -> Option<Cli> {
    let mut args = args;
    match (args.next().as_deref(), args.next(), args.next()) {
        (None, _, _) => Some(Cli::Run { config: None }),
        (Some("--version"), None, _) => Some(Cli::Version),
        (Some("--config"), Some(path), None) => Some(Cli::Run {
            config: Some(PathBuf::from(path)),
        }),
        _ => None,
    }
}

/// Loads the config from `path`, or the default location (falling back to built-in defaults
/// when that file does not exist). Errors are returned as printable messages.
fn load_config(path: Option<PathBuf>, home: &Path) -> Result<Config, String> {
    let explicit = path.is_some();
    let path = path.unwrap_or_else(|| home.join(".config/ln-hub/config.toml"));
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound && !explicit => {
            return Ok(Config::default_for(home));
        }
        Err(e) => return Err(format!("ln-hub: config {}: {e}", path.display())),
    };
    Config::from_toml(&text, home).map_err(|e| format!("ln-hub: config {}: {e}", path.display()))
}

fn main() -> ExitCode {
    let config = match parse_args(std::env::args().skip(1)) {
        None => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
        Some(Cli::Version) => {
            println!("ln-hub {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        Some(Cli::Run { config }) => config,
    };
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        eprintln!("ln-hub: HOME is not set");
        return ExitCode::from(2);
    };
    let config = match load_config(config, &home) {
        Ok(config) => config,
        Err(msg) => {
            eprintln!("{msg}");
            return ExitCode::from(2);
        }
    };
    let mut app = App::new(
        config,
        Box::new(ProcSystem::new(home.join(".cache/ln-hub/jobs.log"))),
    );

    let mut terminal = ratatui::init();
    let result = run(&mut terminal, &mut app);
    ratatui::restore();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ln-hub: {e}");
            ExitCode::FAILURE
        }
    }
}

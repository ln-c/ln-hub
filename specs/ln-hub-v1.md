# Spec: ln-hub v1 — yazi-style TUI hub for ln-* scripts

## Problem & Goal
The ln-* scripts in `~/git/dotfiles/.local/bin` are reachable only through keybindings or by typing
commands. There is no single, discoverable, fast place to browse wallpapers, apply them, change the
font, or toggle monitors.

**Done =** running `ln-hub` in foot opens a yazi-like three-column TUI (modules | items | preview)
in under 50 ms. In it, the user can:
- browse wallpapers in `~/Pictures/Wallpapers` and apply one (via `ln-themectl -w`);
- set the font name and size (via `ln-themectl -f/-s`);
- list monitors and toggle them on or off (via `ln-monitorctl`), with a guard against disabling
  the last enabled monitor.

Colors follow the current wallust palette live. Paths, module order/visibility and keys can be
personalized in `~/.config/ln-hub/config.toml`.

Key qualities: lightweight (single static-ish Rust binary, low RSS), fast (no blocking UI; slow
applies run as background jobs), easy to use (yazi keys), personalizable (config + palette).

## Decisions (user-confirmed)
- Stack: **Rust + ratatui** (crossterm backend). Repo: `~/git/ln-hub` (own repo).
- Backend model: **Hybrid**. Reads go directly to machine-readable sources (`hyprctl monitors all -j`,
  `~/.cache/current_wallpaper`, `~/.config/hypr/env.lua`, `~/.cache/wallust/colors.sh`). Writes go
  through the ln-* scripts (`ln-themectl`, `ln-monitorctl`), so keybindings and the hub share one
  behavior path.
- v1 modules: **Theme** (wallpaper + font/size) and **Monitors**. OVMS is dropped. The
  `ln-ovms-qwen3-8b-npu` script and the zed `ln-hub` ACP agent-server entry have already been
  removed (dotfiles branch `chore/remove-ovms-and-zed-agent`).
- Monitor writes need `ln-monitorctl` fixed first (slice S0).
- Disabling the last enabled monitor is **refused** with a status message.
- Personalization v1: `config.toml` (paths, module order/visibility, keymap overrides) plus live
  wallust palette reload.

## Public Seam Signatures
Crate `ln_hub` (lib + bin). Tests drive `App` like a user: keys go in, a rendered buffer
(ratatui `TestBackend`) and recorded system calls come out.

```rust
pub trait System {
    fn run(&self, prog: &str, args: &[&str]) -> io::Result<Output>; // blocking query
    fn spawn(&self, prog: &str, args: &[&str]) -> io::Result<JobId>; // async apply
    fn poll(&self, job: JobId) -> JobState; // Running | Done | Failed(String)
}

pub struct Config { .. }
impl Config {
    pub fn from_toml(s: &str, home: &Path) -> Result<Config, ConfigError>;
    pub fn default_for(home: &Path) -> Config;
}

pub struct Palette { .. }
impl Palette {
    pub fn parse_colors_sh(s: &str) -> Result<Palette, PaletteError>;
    pub fn get(&self, name: &str) -> Option<Color>; // "background","foreground","color0".."color15"
}

pub struct App { .. }
impl App {
    pub fn new(cfg: Config, sys: Box<dyn System>) -> App;
    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent);
    pub fn tick(&mut self); // poll jobs, reload palette if colors file changed
    pub fn render(&mut self, f: &mut ratatui::Frame);
    pub fn should_quit(&self) -> bool;
}
```

- `Config::default_for(home)` resolves these paths:
  - `wallpaper_dir = home/Pictures/Wallpapers`
  - `current_wallpaper_file = home/.cache/current_wallpaper`
  - `colors_file = home/.cache/wallust/colors.sh`
  - `hypr_env_file = home/.config/hypr/env.lua`
  - `modules = [Theme, Monitors]`
  - default keymap

  `from_toml` overlays these defaults. A `~` in paths expands to `home`.
- **Mockable external boundary: process execution only** (the `System` trait, covering
  `hyprctl`, `ln-themectl` and `ln-monitorctl`). Tests use a `FakeSystem` that records calls and
  returns canned output and job states.
  - The filesystem is **not** mocked. Tests use real temp dirs.
  - Internal modules are never mocked.
- The bin (`src/main.rs`) wires a real `ProcSystem` (std::process; `spawn` = detached child with
  stdout/stderr to `~/.cache/ln-hub/jobs.log`) and the crossterm event loop. It also handles the
  sixel image preview (ratatui-image), which is verified manually in the VM, not by tests.

### Default keymap (yazi-style)
| Action | Default key |
|---|---|
| left / down / up / right | `h` `j` `k` `l` (and the arrow keys) |
| top / bottom | `gg` / `G` |
| apply / toggle | `Enter` |
| filter | `/` (`Esc` clears) |
| font name | `f` |
| font size | `s` |
| help | `?` |
| quit | `q` |

Focus moves between columns with `h`/`l`. `j`/`k` move inside the focused column.

## Module Boundary & Non-Goals
- `App` is the only deep module. It hides:
  - navigation state and the focus model
  - the module registry
  - wallpaper discovery and thumbnail pairing
  - parsing of `env.lua`, `current_wallpaper` and `hyprctl -j`
  - job tracking and status messages
  - palette reload by mtime
  - keymap resolution
- `Config` and `Palette` are small value parsers. They are exposed because they are useful
  standalone and cheap to test.
- **Non-goals (v1):**
  - audio, brightness and media modules
  - OVMS
  - mouse support
  - playing video previews (videos show their `.jpg` thumb)
  - editing `config.toml` from inside the TUI
  - Zed ACP integration
  - Windows/macOS
- **Open questions (deferred):**
  - Distribution: should dotfiles `install.sh` build via `cargo install --locked --git` (which
    adds rust to CORE_PKGS), or download a GitHub release binary? Recommended: a release binary,
    following the existing wallust fallback.
  - Launch keybinding in `hypr/keybindings.lua` (e.g. `SUPER+H` → `foot -e ln-hub`).
  - How deep to recurse into `wallpaper_dir`. v1: top level plus one subdirectory level.

## Ordered Vertical Slices

### Slice 0 (dotfiles prerequisite)
- slice_id: S0-monitorctl-fix
- behavior: `ln-monitorctl <name> on|off` accepts any listed monitor, not only the first one, and
  exits 0 on success. `ini` exits 0. Comparisons are numeric.
- seam: CLI `ln-monitorctl <monitor> [on|off]` | `ln-monitorctl ini`, with `hyprctl` stubbed via
  bats-mock (external boundary).
- test_paths: [/home/ln/git/dotfiles/tests/ln-monitorctl.bats]
- owned_paths: [/home/ln/git/dotfiles/.local/bin/ln-monitorctl]
- acceptance_criteria:
  - Monitors `eDP-1`, `DP-1`; `ln-monitorctl DP-1 off` → hyprctl receives
    `eval hl.monitor({ output = "DP-1", disabled = true })`, exit 0.
  - Same monitors; `ln-monitorctl DP-1 on` → `... output = "DP-1", disabled = false ...`, exit 0.
  - `ln-monitorctl HDMI-A-9 off` → stderr contains `Invalid monitor`, exit 1, no `eval` call.
  - `ln-monitorctl ini` with 2 monitors → disables `eDP-1`, exit 0. With 1 monitor → no `eval`,
    exit 0.
  - BATS runs inside the QEMU VM (bats is not on the host), per AGENTS.md.

### Slice 1
- slice_id: S1-shell-tracer
- behavior: The app renders three columns. The module column lists `Theme` and `Monitors`, and
  `q` quits.
- seam: `App::new(Config::default_for(tmp), FakeSystem)` → `render` on an 80x24 `TestBackend`;
  `handle_key('q')` → `should_quit()`.
- test_paths: [tests/s1_shell.rs, tests/common/mod.rs]
- owned_paths: [Cargo.toml, src/**]
- acceptance_criteria:
  - The rendered buffer contains `Theme` on a row above `Monitors`, both in the leftmost column.
  - `should_quit()` is `false` before and `true` after the key `q`.
  - `j` moves the module highlight from `Theme` to `Monitors`. A second `j` keeps it on
    `Monitors` (no wrap).

### Slice 2
- slice_id: S2-palette-parse
- behavior: The palette is parsed from wallust `colors.sh`.
- seam: `Palette::parse_colors_sh`, `Palette::get`
- test_paths: [tests/s2_palette.rs]
- owned_paths: [src/**]
- acceptance_criteria:
  - Input `background='#170000'\nforeground='#e0d0c0'\ncolor4='#aabbcc'\n` plus color0..15
    gives `get("color4") == Some(Color::Rgb(0xaa,0xbb,0xcc))`.
  - The same input with no `color4` line → `Err(PaletteError::Missing("color4"))`.
  - `color4='zzz'` → `Err(PaletteError::Invalid("color4"))`.
  - Comment lines and `wallpaper='...'` are ignored without error.

### Slice 3
- slice_id: S3-config
- behavior: `config.toml` overrides default paths, module order/visibility and keys.
- seam: `Config::from_toml`, `Config::default_for`, then observed through `App`.
- test_paths: [tests/s3_config.rs]
- owned_paths: [src/**]
- acceptance_criteria:
  - `default_for("/h")`: an App built from it lists wallpapers from `/h/Pictures/Wallpapers`
    (verified with a temp home).
  - `modules = ["Monitors"]` → the rendered module column shows `Monitors` and not `Theme`.
  - `modules = ["Monitors","Theme"]` → `Monitors` is rendered above `Theme`.
  - `[keymap]\nquit = "x"` → `x` quits and `q` no longer quits.
  - `modules = ["Audio"]` → `Err(ConfigError::UnknownModule("Audio"))`.
  - `wallpaper_dir = "~/walls"` with home `/h` → resolves to `/h/walls`.

### Slice 4
- slice_id: S4-wallpaper-list
- behavior: The Theme module lists wallpapers. Video thumbnails are hidden, and the list is
  sorted naturally.
- seam: `App` (focus Theme, `l`) → items column.
- test_paths: [tests/s4_wallpapers.rs]
- owned_paths: [src/**]
- acceptance_criteria:
  - Wallpaper dir with `img10.png`, `img2.png`, `b.mp4`, `b.jpg`, `notes.txt` and
    `images/c.jpg` → items render in the order `b.mp4`, `img2.png`, `img10.png`, `images/c.jpg`.
    `b.jpg` (thumb of `b.mp4`) and `notes.txt` are absent.
  - An empty or missing dir → the items column shows `No wallpapers in <dir>`. No panic.

### Slice 5
- slice_id: S5-current-wallpaper
- behavior: The currently applied wallpaper is marked and preselected.
- seam: `App` render.
- test_paths: [tests/s5_current.rs]
- owned_paths: [src/**]
- acceptance_criteria:
  - `current_wallpaper` file contains `<dir>/img2.png\n` → the `img2.png` row contains `●` and is
    highlighted on entering Theme.
  - A missing file → no `●`, and the highlight is on the first item.

### Slice 6
- slice_id: S6-apply-wallpaper
- behavior: `Enter` applies the selected wallpaper as a background job and shows its progress.
- seam: `App::handle_key(Enter)`, `App::tick`, and the `FakeSystem` call log / job states.
- test_paths: [tests/s6_apply.rs]
- owned_paths: [src/**]
- acceptance_criteria:
  - `Enter` on `img2.png` → exactly one `spawn("ln-themectl", ["-w", "<abs>/img2.png"])`. The
    status bar shows `Applying img2.png…`.
  - A second `Enter` while the job is `Running` → no second spawn.
  - Job set to `Done`, then `tick()` → the status shows `Applied img2.png`.
  - `Failed("exit 1")`, then `tick()` → the status shows `Apply failed: exit 1`.
  - `spawn` returns an `io::Error` "not found" → the status shows
    `ln-themectl not found: not found`. No panic.

### Slice 7
- slice_id: S7-preview-meta
- behavior: The preview column shows details of the hovered wallpaper. The image itself is drawn
  only by the bin.
- seam: `App` render.
- test_paths: [tests/s7_preview.rs]
- owned_paths: [src/**]
- acceptance_criteria:
  - Hovering `img2.png` (2048 bytes) → the preview shows `img2.png` and `2.0K`.
  - Hovering `b.mp4` with `b.jpg` beside it → the preview shows `video` and `thumb: b.jpg`.

### Slice 8
- slice_id: S8-font
- behavior: Shows the current font name and size, and sets them through `ln-themectl`.
- seam: `App` keys `f` / `s`, typed text, `Enter`, `Esc`.
- test_paths: [tests/s8_font.rs]
- owned_paths: [src/**]
- acceptance_criteria:
  - `env.lua` with `hl.env("FONT_NAME", "Terminess Nerd Font Propo")` and
    `hl.env("FONT_SIZE", "16")` → the Theme view shows `Terminess Nerd Font Propo` and `16`.
  - A missing `env.lua` → shows the defaults `Terminess Nerd Font Propo` / `16`.
  - `f`, typing `JetBrainsMono Nerd Font`, then `Enter` →
    `spawn("ln-themectl", ["-f", "JetBrainsMono Nerd Font"])`.
  - `s`, typing `18`, then `Enter` → `spawn("ln-themectl", ["-s", "18"])`.
  - `s`, typing `abc`, then `Enter` → no spawn, and the status shows `Font size must be 6-72`.
    The same happens for `5` and `73`.
  - `Esc` during input → no spawn, and the prompt closes.

### Slice 9
- slice_id: S9-monitor-list
- behavior: The Monitors module lists monitors from `hyprctl -j`.
- seam: `App` with the `FakeSystem` `run("hyprctl", ["monitors","all","-j"])` canned output.
- test_paths: [tests/s9_monitors.rs]
- owned_paths: [src/**]
- acceptance_criteria:
  - Canned JSON with `eDP-1` (2880x1800@120, `disabled:false`) and `DP-1` (2560x1440@60,
    `disabled:true`) → rows `eDP-1  2880x1800@120  on` and `DP-1  2560x1440@60  off`.
  - `run` returns an error → the items column shows `hyprctl unavailable`. No panic.

### Slice 10
- slice_id: S10-monitor-toggle
- behavior: `Enter` toggles a monitor through `ln-monitorctl`, refuses to disable the last
  enabled monitor, and refreshes the list after the job.
- seam: `App` with the `FakeSystem` call log.
- test_paths: [tests/s10_toggle.rs]
- owned_paths: [src/**]
- acceptance_criteria:
  - Both monitors on; `Enter` on `DP-1` → `spawn("ln-monitorctl", ["DP-1","off"])`.
  - `DP-1` off; `Enter` on `DP-1` → `spawn("ln-monitorctl", ["DP-1","on"])`.
  - Only `eDP-1` on; `Enter` on `eDP-1` → no spawn, and the status shows
    `Refusing to disable the last enabled monitor`.
  - The job is `Done`, then `tick()` → `hyprctl monitors all -j` is called again.

### Slice 11
- slice_id: S11-palette-live
- behavior: UI colors come from the wallust palette and reload when `colors.sh` changes.
- seam: `App::render`, `App::tick`, with a real temp `colors.sh`.
- test_paths: [tests/s11_palette_live.rs]
- owned_paths: [src/**]
- acceptance_criteria:
  - `color4='#aabbcc'` → the highlighted row's cell bg is `Rgb(0xaa,0xbb,0xcc)`.
  - Rewrite the file with `color4='#112233'` and a newer mtime, then `tick()` → the highlight bg
    is `Rgb(0x11,0x22,0x33)`.
  - A missing or invalid `colors.sh` → the built-in fallback palette is used. No panic.

### Slice 12
- slice_id: S12-filter
- behavior: `/` filters the items column as you type.
- seam: `App` keys.
- test_paths: [tests/s12_filter.rs]
- owned_paths: [src/**]
- acceptance_criteria:
  - Items `frieren.png`, `cat.mp4`, `img2.png`; `/`, typing `fri` → only `frieren.png` is
    rendered.
  - `Esc` → all 3 are rendered again.
  - The filter is case-insensitive: `FRI` also matches.

### Slice 13
- slice_id: S13-bin
- behavior: The binary runs: `--version`, a real `ProcSystem`, the terminal loop, and the sixel
  preview.
- seam: CLI `ln-hub [--version|--config <path>]`.
- test_paths: [tests/s13_cli.rs]
- owned_paths: [src/main.rs, src/**]
- acceptance_criteria:
  - `ln-hub --version` → stdout `ln-hub 0.1.0`, exit 0.
  - `ln-hub --config /nonexistent.toml` → stderr contains `config`, exit 2.
  - `ln-hub --config <file with modules=["Audio"]>` → stderr contains `unknown module: Audio`,
    exit 2.
  - Manual check in the QEMU VM (not automated):
    - startup to first frame under 50 ms (`hyperfine`-style);
    - sixel preview visible in foot;
    - applying a wallpaper restarts waybar and keeps the UI responsive.

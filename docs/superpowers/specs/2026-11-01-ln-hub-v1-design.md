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
- VM evidence: from S23 on, the VM screenshots (`tests/manual/**/*.png`) live in Git LFS. The 75
  PNGs already tracked are converted in ONE new commit with `git lfs migrate import --no-rewrite`
  (no history rewrite, no force-push; the old blobs stay in history). Text dumps stay plain files.
- VM scripts (S24): the VM runs in snapshot mode only, so the base disk is never updated. On each
  boot the fixed `ln-*` scripts from the 9p-shared `~/git/dotfiles/.local/bin` are copied into the
  guest's `~/.local/bin` (or that directory is prepended to `PATH`), so the guest never runs an
  outdated script.
- The sixel encode moves off the UI thread with `ratatui_image::thread::ThreadProtocol` (S22); no
  extra cargo feature is needed.
- S19-S24 build order (plan `docs/superpowers/plans/2026-11-07-s19-s24-followups.md`, confirmed
  2026-11-07): S20, S19, S21, S22, S23, S24. S20 goes first so every later slice can require
  `cargo fmt --check` to exit 0.
- S22 details (confirmed 2026-11-07): the preview is blank while an encode is pending; an
  `LN_HUB_TIMING` path that cannot be opened turns timing off silently; failed encodes are logged
  too; the loop polls every 10 ms while an encode is pending (100 ms otherwise); `ratatui::init()`
  is kept for its panic hook and screen setup, and the bin builds its own `Terminal` over a
  `BufWriter` around a sixel-detecting stdout wrapper.
- S23 commits (confirmed 2026-11-07): the LFS migrate stays ONE commit; the S23 status flip is a
  separate commit after it.

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
    pub fn init_started(&mut self); // status shows Initializing… while the startup query runs (S16)
    pub fn init_finished(&mut self, now: Instant); // Initializing… → Ready for ~1 s or until the first key (S16)
    pub fn tick_at(&mut self, now: Instant); // tick() == tick_at(Instant::now()) (S16)
    pub fn render(&mut self, f: &mut ratatui::Frame);
    pub fn should_quit(&self) -> bool;
    pub fn preview_image(&self) -> Option<PreviewImage>; // as of the last render (S13)
}

pub struct PreviewImage { pub path: PathBuf, pub area: Rect } // hovered image / video thumb
pub struct ProcSystem { .. } // real System; ProcSystem::new(log: PathBuf) (S13)
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
  - The sixel resize+encode runs on a worker thread through `ratatui_image::thread::ThreadProtocol`
    (S22), so the UI thread never blocks on an encode.
  - The opt-in `LN_HUB_TIMING=<file>` env var makes the bin append `encode <cols>x<rows> <ms>ms` and
    `draw <ms>ms` lines to `<file>` (S22). It is bin-only and not part of the lib seam.

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
  - A custom sixel encoder: the dither and the colour count are hardcoded in ratatui-image, so
    changing them would need a fork. Not planned; decide after the S24 timing shows whether the
    encode or the terminal write dominates.
  - Terminal write cost: if S24 finds that the `terminal.draw` flush of the sixel bytes dominates
    the encode, that is an open question, not fixed in S19-S24.
  - `[profile.dev.package."*"] opt-level` for debug builds: not needed, because the VM and the
    distribution use release. A debug cold encode is ~940 ms against ~20 ms in release.

## Slices

Slices S0–S18 are archived one file per slice under `docs/superpowers/plans/archive/ln-hub-v1/`
(front matter links back to this design). From S19 on, slices are tasks in a plan under
`docs/superpowers/plans/` (S19–S24: `2026-11-07-s19-s24-followups.md`).

| slice | behavior | file | status |
|---|---|---|---|
| S0-monitorctl-fix | ln-monitorctl accepts any listed monitor and exits 0 (dotfiles prerequisite) | [s00-monitorctl-fix.md](../plans/archive/ln-hub-v1/s00-monitorctl-fix.md) | unverified |
| S1-shell-tracer | Three-column shell; module column lists Theme and Monitors; `q` quits | [s01-shell-tracer.md](../plans/archive/ln-hub-v1/s01-shell-tracer.md) | green |
| S2-palette-parse | Palette parsed from wallust `colors.sh` | [s02-palette-parse.md](../plans/archive/ln-hub-v1/s02-palette-parse.md) | green |
| S3-config | `config.toml` overrides paths, module order/visibility and keys | [s03-config.md](../plans/archive/ln-hub-v1/s03-config.md) | green |
| S3.1-config-hardening | Config hardening: reject broken or typo-ridden configs | [s03.1-config-hardening.md](../plans/archive/ln-hub-v1/s03.1-config-hardening.md) | green |
| S4-wallpaper-list | Theme module lists wallpapers (thumbs hidden, natural sort) | [s04-wallpaper-list.md](../plans/archive/ln-hub-v1/s04-wallpaper-list.md) | green |
| S4.1-wallpaper-list-hardening | Wallpaper list hardening: media types, case-insensitive sort, grouping | [s04.1-wallpaper-list-hardening.md](../plans/archive/ln-hub-v1/s04.1-wallpaper-list-hardening.md) | green |
| S5-current-wallpaper | Current wallpaper marked and preselected | [s05-current-wallpaper.md](../plans/archive/ln-hub-v1/s05-current-wallpaper.md) | green |
| S6-apply-wallpaper | `Enter` applies the selected wallpaper as a background job | [s06-apply-wallpaper.md](../plans/archive/ln-hub-v1/s06-apply-wallpaper.md) | green |
| S7-preview-meta | Preview column shows details of the hovered wallpaper | [s07-preview-meta.md](../plans/archive/ln-hub-v1/s07-preview-meta.md) | green |
| S8-font | Font name and size shown and set through `ln-themectl` | [s08-font.md](../plans/archive/ln-hub-v1/s08-font.md) | green |
| S9-monitor-list | Monitors module lists monitors from `hyprctl -j` | [s09-monitor-list.md](../plans/archive/ln-hub-v1/s09-monitor-list.md) | green |
| S10-monitor-toggle | `Enter` toggles a monitor, refuses the last enabled one, refreshes after the job | [s10-monitor-toggle.md](../plans/archive/ln-hub-v1/s10-monitor-toggle.md) | green |
| S11-palette-live | UI colors follow the wallust palette and reload live | [s11-palette-live.md](../plans/archive/ln-hub-v1/s11-palette-live.md) | green |
| S12-filter | `/` filters the items column as you type | [s12-filter.md](../plans/archive/ln-hub-v1/s12-filter.md) | green |
| S13-bin | The binary: `--version`, real `ProcSystem`, terminal loop, sixel preview | [s13-bin.md](../plans/archive/ln-hub-v1/s13-bin.md) | green |
| S14-picker-query | Terminal capability query with a hard deadline; no freeze, no eaten keys | [s14-picker-query.md](../plans/archive/ln-hub-v1/s14-picker-query.md) | green |
| S15-monitor-popup | `Enter` on a monitor opens a confirm popup overlay | [s15-monitor-popup.md](../plans/archive/ln-hub-v1/s15-monitor-popup.md) | green |
| S16-init-status | Status row shows `Initializing…` during the startup query, then `Ready` until the first key or ~1 s | [s16-init-status.md](../plans/archive/ln-hub-v1/s16-init-status.md) | green |
| S17-help-status-overlap | The right-aligned `Font:` status never overprints the help line or other left status text | [s17-help-status-overlap.md](../plans/archive/ln-hub-v1/s17-help-status-overlap.md) | green |
| S18-review-followups | S14/S15 review follow-ups: sturdier pty tests, sixel-reply pty test, popup and query-thread refactors | [s18-review-followups.md](../plans/archive/ln-hub-v1/s18-review-followups.md) | green |
| S19-popup-tidy | `Popup::new` with private fields, a narrow `Confirm` payload, no unreachable arm (refactor, no behavior change) | [plan, Task 2](../plans/2026-11-07-s19-s24-followups.md) | todo |
| S20-test-rustfmt | `cargo fmt --check` exits 0 again (five layout hunks in two test files) | [plan, Task 1](../plans/2026-11-07-s19-s24-followups.md) | green |
| S21-pty-helper-dedup | The pty tests share one `find` / `wait_for` / `fail` in `tests/common/pty.rs` | [plan, Task 3](../plans/2026-11-07-s19-s24-followups.md) | todo |
| S22-sixel-off-ui-thread | Sixel resize+encode on a worker thread (`ThreadProtocol`); opt-in `LN_HUB_TIMING` log | [plan, Task 4](../plans/2026-11-07-s19-s24-followups.md) | todo |
| S23-evidence-lfs | VM evidence PNGs move to Git LFS (one new commit, no history rewrite) | [plan, Task 5](../plans/2026-11-07-s19-s24-followups.md) | todo |
| S24-vm-check | VM: run the S0 bats suite (flip S0 to green) and check S22's responsiveness and encode/draw timing | [plan, Task 6](../plans/2026-11-07-s19-s24-followups.md) | todo |

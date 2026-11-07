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

### Slice 3.1 (review follow-up)
- slice_id: S3.1-config-hardening
- behavior: `from_toml` rejects configs that would leave `App` in a broken state or hide typos.
- seam: `Config::from_toml`
- test_paths: [tests/s3_1_config_hardening.rs]
- owned_paths: [src/**]
- acceptance_criteria:
  - `wallpaper_dir = "~//walls"` with home `/h` → `/h/walls` (not `/walls`).
  - `modules = []` → `Err(ConfigError::NoModules)`.
  - `modules = ["Theme", "Theme"]` → `Err(ConfigError::DuplicateModule("Theme"))`.
  - `wallpaper_dr = "x"` (unknown top-level key) → `Err(ConfigError::Parse(msg))` with `msg`
    containing `wallpaper_dr`.
  - `[keymap]\nquite = "x"` (unknown keymap key) → `Err(ConfigError::Parse(msg))` with `msg`
    containing `quite`.
- Deferred to S5+: key-conflict validation and an `Action` keymap table. Moved to S12.

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
  - (Deferred from S3) An `App` built from `Config::default_for(home)` lists the wallpapers in
    `<home>/Pictures/Wallpapers`.

### Slice 4.1 (review follow-up)
- slice_id: S4.1-wallpaper-list-hardening
- behavior: The wallpaper list has a defined set of media types, sorts case-insensitively, keeps
  each subdirectory's files together, and only hides a `.jpg` when a video shares its stem.
- seam: `App` (focus Theme, `l`) → items column.
- test_paths: [tests/s4_1_wallpapers_hardening.rs, tests/s4_1_non_utf8.rs]
- owned_paths: [src/**]
- acceptance_criteria:
  - Media types: images `png jpg jpeg webp gif`, videos `mp4 webm mkv`, extension matched
    case-insensitively. Dir with `Zebra.png`, `apple.png`, `Mango.PNG`, `clip.WEBM`, `pic.webp`,
    `x.jpeg`, `y.gif`, `zoo.mkv`, `doc.pdf` → items in the order `apple.png`, `clip.WEBM`,
    `Mango.PNG`, `pic.webp`, `x.jpeg`, `y.gif`, `Zebra.png`, `zoo.mkv`; `doc.pdf` absent.
  - Dir with `solo.jpg`, `b.mp4`, `b.jpg` → `b.mp4` then `solo.jpg`; `b.jpg` absent.
  - Dir with `a1/x.png`, `a01/y.png`, `a1/z.png` → `a01/y.png`, `a1/x.png`, `a1/z.png`
    (subdirectories sorted by name, each subdirectory's files contiguous).
  - A file whose name is not valid UTF-8 (e.g. bytes `bad\xff.png`) is skipped; `good.png`
    beside it is still listed. (Its path could not round-trip through a display name.)
- Refactor (no behavior change, after GREEN): `wallpaper::discover` returns `Vec<Wallpaper>`
  (`name`, `path`, `kind`, `thumb`) so S5 and S7 reuse the pairing; classify each file once.
  Extracting a Theme items pane from `App` is deferred to S5, where the cursor arrives (done in
  S5: `src/theme_pane.rs`).

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
  - (Review follow-up) A `current_wallpaper` path not in the list (e.g. same file name outside
    the wallpaper dir) → no `●`, highlight on the first item.
- Carried notes: resolved, or moved to S10–S13 (audited 2026-11-07).

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
  - (Review follow-up, S5 carry-over) Job `Done`, then `tick()` → `current_wallpaper` is
    re-read, so `●` moves to the applied wallpaper and the cursor stays put. `Failed` → no
    re-read.
- Carried notes: resolved, or moved to S10–S13 (audited 2026-11-07).

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
  - (Review follow-up) Sizes are `ls -h` style and round up: 10 → `10`, 1500 → `1.5K`,
    1025 → `1.1K`, 10240 → `10K`, 1048576 → `1.0M`. An image shows `image` and no
    `thumb:`. The preview is empty until items are focused, and also when the list is empty.
- Carried notes: resolved, or moved to S10–S13 (audited 2026-11-07).

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
  - (Review follow-up) Pinned:
    - A Done job re-reads env.lua; a Failed job does not.
    - Every key typed into an open prompt is text, including the quit key.
    - `f` / `s` work only with the Theme items focused.
    - `Enter` in a prompt waits (no spawn, prompt kept) while a job is in flight.
    - `Backspace` edits the prompt text.
    - Name and size are trimmed, and an empty name does nothing.
    - The prompts are labelled `Font name:` / `Font size:`.
    - Font jobs show `Applying font size 18…` / `Applied font size 18` (and the same for `font name`).
    - The font line is hidden until the items column is focused.
- Carried notes: resolved, or moved to S10–S13 (audited 2026-11-07).

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
- review follow-up (pinned in tests/s9_monitors.rs):
  - Unparseable `hyprctl` output also shows `hyprctl unavailable`. This covers a non-zero
    exit, because `ProcSystem::run` returns `Ok` with empty stdout.
  - `hyprctl` runs only when `l` focuses Monitors, never at start-up or on Theme. Pressing `l`
    again re-queries.
  - `f`, `s` and Enter do nothing on Monitors.
  - Carried from S8: an open prompt survives `tick` finishing a job. The `Applied …` status
    shows after Esc. The font line is absent on Monitors.
- Carried notes: resolved, or moved to S10–S13 (audited 2026-11-07).

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
- Carried notes (from S5–S9; audited 2026-11-07):
  - (S9) Use one focus check. `theme_focused()`, `theme_active` and `in_items` duplicate
    `focused_module()`, so use `focused_module() == Some(Module::Theme)`. The monitor cursor
    and Enter need the same check for Monitors.
  - (S9) `MonitorsPane` uses `Some(vec![])` for both "not loaded" and "no monitors". Switch to
    a `Loaded | Unavailable` enum if a `No monitors` message is needed. Unpinned so far:
    - one entry missing a field makes the whole list unavailable;
    - an empty `[]` shows a blank list.
  - (S6) Generalise `Apply` into one running-job → status-message type, now that a second job
    kind (`ln-monitorctl`) exists.
  - (S6/S8) Every spawn error reads `ln-themectl not found: …`, which is misleading for
    errors other than `NotFound`. Split the wording (e.g. `Failed to start <prog>: …`) behind
    a RED test. This also applies to the new `ln-monitorctl` spawn.
  - (S6) The status is never cleared, and Enter while a job is running is silently ignored.
    Decide both here, because Enter gains a second meaning.
  - (S6) Unpinned: Enter with items unfocused (in the module column) and Enter on an empty
    wallpaper list. Enter on Monitors before S10 was pinned as a no-op in S9 and is now
    replaced by the toggle.
  - (S6, tests) Move the duplicated test helpers into `tests/common`:
    - `items_hits`, now in s4, s4_1, s5 and s6;
    - `name_style` and `assert_only_highlighted`, in s5 and s6;
    - `row_of` together with a `name_cell` helper;
    - the shared `l`,`j` setup in `app_on_img2` and `app_current_img1_cursor_img2`.
    Do this when s10 needs a highlight or row lookup.
- review follow-up (pinned in tests/s10_toggle.rs):
  - `j`/`k` move a highlighted monitor cursor, clamped at both ends. With modules
    `[Monitors, Theme]`, `j` inside the Monitors items does not move the module column.
  - The status reads `Applying DP-1 off…` → `Applied DP-1 off`. `Failed` shows
    `Apply failed: <reason>` and does not re-query `hyprctl`.
  - A second Enter while a job is running does nothing. Enter works again after `Done`.
  - Spawn errors: `NotFound` → `<prog> not found: …`, other kinds → `Failed to start <prog>: …`,
    for both `ln-themectl` and `ln-monitorctl`.
  - The cursor clamps when a refresh shrinks the list. A refusal leaves no job in flight. A
    cursor starting on a disabled first monitor toggles it `on`.
  - Characterisation: Enter with items unfocused, on an empty wallpaper list, and on Monitors
    with no usable list does nothing.
- Carried notes resolved here: one focus check (`focused_module()`), `Apply` generalised to
  `Job` with an `OnDone` reload, the spawn-error wording split, the Enter-unfocused/empty
  pins, and the test-helper dedup into `tests/common` (except the two-line `l`,`j` setup in
  s6, see S12).
- Decisions:
  - The status is not cleared; it stays until the next message. Enter while a job is running
    stays silent. Re-check both in the S13 manual VM run.
  - The toggle wording reuses `Applying`/`Applied`/`Apply failed` with `<name> <on|off>`.
- Moved on:
  - (to S13) `Loaded | Unavailable` enum and a `No monitors` message; still unpinned: a
    missing field makes the whole list unavailable, and `[]` shows a blank list.
  - (to S12) Replace the repeated `in_theme`/`in_monitors` guard arms with per-pane dispatch;
    one shared `Cursor` type for the duplicated clamp logic; optionally a `Power { On, Off }`
    enum instead of `action: &'static str`.
  - (accepted) `tick` reloads monitors even after leaving the Monitors view. Harmless.

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
- Carried notes (from S6; audited 2026-11-07):
  - (S6) `ThemePane`'s private `load_current` only wraps `wallpaper::read_current`. Inline it
    unless the reload path gains logic. Decide when `tick` gains palette reload.
- review follow-up (pinned in tests/s11_palette_live.rs):
  - The highlight is `background` text on a `color4` bar, in the module column and the items.
    The fg follows a reload too.
  - The fallback is the pre-S11 look: `REVERSED` with fg and bg `Reset`. It is used when the
    file is missing, has an invalid value, or misses a key at start (an empty file is pinned
    only as a rewrite after a good load).
  - `tick` reloads with no job in flight; `render` never reads the file. The file is reread
    only when its mtime differs from the last one seen, including an mtime already seen
    after a reload.
  - A reload of an invalid, empty or deleted file keeps the last good palette (wallust may be
    mid-write). A later valid file recovers. A file missing or invalid at start is picked up
    once a valid one appears.
- Carried note resolved: `load_current` inlined, because the palette reload lives in
  `LivePalette` (src/palette.rs) and `ThemePane::reload` gained nothing.
- Decisions:
  - "Differs from the last seen mtime", not "newer": a replaced file with an older mtime also
    reloads. Unpinned.
  - Accepted: an invalid file's mtime is marked seen, so if it is completed within the same
    mtime it is missed until the next write. Rare with ns mtimes.
  - Only the list highlight follows the palette; borders, titles and status keep terminal
    colors. Pass one `Styles` value to the panes instead of a bare `Style` when a second
    palette color is used (S12 or later).
- Follow-ups (non-blocking, from review):
  - `Palette::highlight` reads `background`/`color4` via hard-coded indices into `NAMES`
    (guarded by the s11 tests); prefer named lookups or a const assert, and fold the second
    `impl Palette` block into the first.
  - The keep-last-good tests pin only bg, and no test checks the palette highlight in the
    Monitors items; add those asserts when the palette code is touched again.

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
- Carried notes (from S3.1–S9; audited 2026-11-07). Do the keymap work before the filter:
  - (S3.1/S5/S6) Add an `Action` keymap table with key-conflict validation (`quit = "j"`). The
    module column has no `h`/`k` and there are no arrow keys (keymap table, line ~89/97).
  - (S6) The quit arm matches first, so a `q` typed into the filter would quit. Open prompts
    already take priority (pinned in S8). The filter must work the same way.
  - (S6/S8) Route keys through `ThemePane::handle_key -> Option<PaneAction>` while items are
    focused. Move `f`/`s` and the prompt (`Prompt`, `handle_prompt_key`, `submit_prompt`)
    out of `App` into the Theme pane.
  - (S8) The filter is the third prompt. Make `Prompt` a `{ kind, text }` struct to end the
    `FontName | FontSize` switch, and build the themectl args and status label from one
    `Prompt::command()`. If editing grows (cursor, paste), extract a `TextInput`.
  - (S9) Sticky focus: `j` in the Monitors items falls through to the module column. With
    Monitors listed before Theme, this would show Theme items without `focus_items`. Fix it
    together with `h` / focus handling.
  - (S9) Consider a `Pane` trait if the filter needs dispatch across panes. With two panes, a
    `match` is enough.
  - (S10) Replace the repeated `in_theme` / `in_monitors` guard arms in `App::handle_key` with
    per-pane dispatch. Extract one `Cursor` type for the clamp logic duplicated in
    `ThemePane` and `MonitorsPane`. Optionally use a `Power { On, Off }` enum instead of
    `Toggle::Run { action: &'static str }`.
  - (S10, tests) Test-helper nits in `tests/common`:
    - move the `draw(app)` and `assert_status(app, …)` wrappers duplicated in s6 and s10;
    - share the `l`,`j` setup of `app_on_img2` and `app_current_img1_cursor_img2` in s6;
    - add a doc line on `items_hits` (starts in the column) versus `items_hit_within` (fully
      inside);
    - rename the `Style` alias to `CellStyle` so it does not clash with `ratatui::style::Style`.
- Carried notes resolved here:
  - Keymap: `Action` table in src/pane.rs (`h j k l f s`, arrows, `/`, `Esc`). The quit key is
    validated (`ConfigError::KeyConflict`); it may not be `h j k l f s` or `/`. `h`/`k` and the
    arrows work in the module column.
  - Keys go to `ThemePane::handle` / `MonitorsPane::handle` with `Option<PaneAction>`. The
    panes name their `Program`, so `App::run` no longer infers it from focus. `f`/`s` and the
    prompts live in the Theme pane; `Prompt { kind, text }` with one `Prompt::command()`.
  - Open prompts take every key, the quit key included (the filter too).
  - Sticky focus: already resolved; pinned by a characterisation test.
  - One `Cursor` type, `Power { On, Off }`, and the shared test helpers in `tests/common`.
  - No `Pane` trait: a `match` over two panes is enough.
- Decisions:
  - The filter is Theme-only. `/` does nothing in the module column or in Monitors.
  - The status row shows `Filter: <text>` while the filter is being typed.
  - Every printable key is text while typing; non-text keys (arrows, …) change nothing.
  - The filter is a case-insensitive substring match on the name. A text change moves the
    cursor to the first match.
  - `Enter` closes the input and keeps the filter (and applies nothing). Reopening `/` shows the
    kept text and leaves the cursor where it is.
  - `Esc` (typing or not) clears the filter and keeps the highlighted wallpaper highlighted, or
    row 0 if nothing matched. `h`/`Left` out of the items also clear it.
- Follow-ups (non-blocking, from review):
  - src/config.rs depends on `pane::Action` for the quit validation; a small `keymap` module
    would decouple them.
  - The idle/in-flight policy is split: an `idle` bool into `handle_prompt_key` and an
    `Apply` guard in `App`. Untested: opening a filter or font prompt while a job is in flight.
  - `ThemePane::visible()` allocates and lowercases on every call; cache it if lists grow.
  - `Action` mixes app-level (Quit, Left/Right focus) and pane-level variants; fine with two
    panes.
  - `PromptKind` still switches in `line`/`command`/`handle_prompt_key`; extract a `TextInput`
    if editing grows.
  - Cosmetic: `MonitorsPane::unfocus` is an empty method kept for the exhaustive match;
    `toggle`'s `match m.disabled` reads worse than `if`/`else`; the Esc arm must clear before
    closing the prompt (add a comment); the two `if filtering { cursor = 0 }` copies and the
    two `Prompt::new(kind, String::new())` calls could be folded.
  - Untested: `.saturating_sub(1)` in the module-column `Down` (empty module list); `Esc` from a
    reopened filter prompt.
  - `gg`/`G` (top/bottom) and `?` (help) from the default keymap are in no slice yet.

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
- Carried notes (from S5–S9; audited 2026-11-07):
  - (S7) The bin loop: `src/main.rs` never calls `App::tick` and redraws only on a key event,
    so in the real binary an apply stays on `Applying…`, `●` never moves, and the monitor
    list never refreshes. Use `event::poll(timeout)`, then `tick()` and redraw.
  - (S7) Stop `render_preview` calling `fs::metadata` on every frame. Capture `size` once in
    `discover` (on `Wallpaper`) or cache it in `ThemePane`.
  - (S7) The sixel preview is the second consumer of `human_size` / `preview_lines`. Move them
    into a small `preview.rs`, and consider a `Kind::label()` at the same time. Optional
    nits:
    - `preview_lines` could return the joined text or a `Vec<Line>`;
    - the interior-rows comment in `tests/s7_preview.rs` near `empty_list_preview_is_blank`
      slightly misdescribes the slice it takes.
  - (S9) Monitor rows are wider than the 18-cell items interior at 80 columns and get
    truncated. The tests draw at 120x24. Check this in foot.
  - (S6) Paths go to `spawn` via `to_string_lossy`, a limit of the `&[&str]` API.
  - Accepted, unpinned behaviour. Pin each item if its code is touched again:
    - (S5) `wallpaper::read_current` strips only the line terminator, so a path keeps its own
      trailing spaces. An empty or non-UTF-8 file means no current wallpaper.
    - (S7) When `fs::metadata` fails (e.g. a broken symlink), the size line is omitted.
    - (S7) The unit rollover in `human_size`: 1048575 bytes → `1.0M`, not `1024K`.
    - (S8) In `font.rs` `env_value`, a duplicate `hl.env` key resolves to the first line. The
      exact `hl.env("K", "v")` spacing is required and escapes are not handled. That is fine
      for a file `ln-themectl` writes.
    - (S8) `parse_size` accepts `+16`, which is harmless because the size is re-serialised.
  - (S10) `MonitorsPane` uses `Some(vec![])` for both "not loaded" and "no monitors". Switch to
    a `Loaded | Unavailable` enum if a `No monitors` message is wanted. Still unpinned: one
    entry missing a field makes the whole list unavailable, and `[]` shows a blank list.
  - (S10) Manual check: the status is never cleared and Enter while a job runs is silent.
    Confirm this reads well in foot.

---
spec: ../../specs/ln-hub-v1.md
slice_id: S13-bin
status: green
---

### Slice 13
- slice_id: S13-bin
- behavior: The binary runs: `--version`, a real `ProcSystem`, the terminal loop, and the sixel
  preview.
- seam: CLI `ln-hub [--version|--config <path>]`.
- test_paths: [tests/s13_cli.rs, tests/s13_proc.rs, tests/s13_keys.rs,
  tests/s13_preview_image.rs]
- owned_paths: [src/main.rs, src/**, Cargo.toml]
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
  - (S12) The default keymap's `gg`/`G` (top/bottom of the focused column) and `?` (help) are
    not implemented. Add them as `Action`s in src/pane.rs: `G` and `?` join the built-in keys
    the quit key may not use; `gg` needs a pending-`g` state in `App` (a lone `g` then another
    key does nothing). While a prompt is open they are text. Help can be a status-row or
    overlay list of the active keys.
- Carried notes resolved here:
  - Bin loop: `event::poll(100ms)`, then `tick()` and redraw every iteration.
  - `fs::metadata` runs once in `discover` (`Wallpaper.size: Option<u64>`); the size line is
    still omitted when it fails.
  - `ProcSystem` lives in the lib (src/proc.rs); `spawn` appends stdout/stderr to
    `~/.cache/ln-hub/jobs.log`, creating its dirs.
  - `gg`/`G`/`?` are `Action::Top/Bottom/Help`; one `Cursor::navigate` serves both panes.
  - Default config: without `--config`, `$HOME/.config/ln-hub/config.toml` is read, and
    `Config::default_for` is used if that file does not exist.
- Decisions:
  - The CLI is hand-parsed (`--version`, `--config <path>`). Usage errors, config errors and
    a missing `HOME` exit 2 with `ln-hub: …` on stderr; a terminal error exits 1.
  - An unusable `jobs.log` falls back to null sinks; the job still runs.
  - `poll` reports `Failed("exit N")`, or `Failed("killed by signal")` when there is no exit
    code (unpinned).
  - A lone `g` swallows the next key, quit and `Esc` included; `gG` does nothing. `g`, `G`,
    `?` are text in prompts and may not be the quit key.
  - `?` shows the key help in the status row:
    `h/j/k/l move  gg/G top/bottom  ⏎ apply  / filter  f font  s size  ? help  <quit> quit`.
  - Sixel preview (bin-only src/image_view.rs, ratatui-image 11, `image` png/jpeg/webp/gif):
    the area is `App::preview_image()` below the preview text. The terminal picker is queried
    after the first frame (halfblocks fallback). Decoding runs on one worker thread that
    coalesces to the newest request (older ones answered `Skipped`), downscales sources over
    1600 px, and turns a panic into `Failed`. Protocols sit in an 8-entry LRU; failed decodes
    are cached for the session. `Resize::Scale` with a Triangle filter.
- Deferred:
  - (S7) `human_size` / `preview_lines` stay in theme_pane.rs; the sixel view does not use
    them, so there is no second consumer yet. Same for `Kind::label()`.
  - (S10) `Loaded | Unavailable` and a `No monitors` message.
- Follow-ups (non-blocking, from review):
  - The loop redraws every 100 ms when idle; a dirty flag (key, decode reply, tick change,
    resize) would avoid it.
  - Sixel resize/encode runs on the UI thread at the first render of each image and on area
    change; use ratatui-image's threaded protocol if it stalls input in foot.
  - `App::render` publishes `preview_image` through state; returning it from `render` would
    drop the ordering coupling.
  - Key knowledge sits in three places (`CHAR_KEYS`, the `g` prefix in `App`, the help
    string); a `BINDINGS` table and a small key parser would unify them. `?` overwrites an
    `Applied…` status.
  - `ImageView::new` queries the terminal itself; taking a `Picker` would make it testable
    with `TestBackend`. No unit test for the LRU.
  - `ProcSystem` never prunes finished `Child` handles; `jobs.log` has no per-job header and
    grows without bound.
  - `parse_args` could be a `while let` loop with specific messages and `--help`.
- Manual check in the QEMU VM (foot), in addition to the acceptance list. Both lists are carried
  over to the VM test in the dotfiles repo:
  - the sixel image disappears when moving off an image and redraws correctly on resize;
  - j/k through many large images stays responsive;
  - the help line: fits at 80 columns, truncates below 80;
  - monitor rows at 80 columns (S9) and the status / silent Enter while a job runs (S10);
  - a terminal that does not answer the picker query starts with halfblocks, not a hang.
- Manual VM run (2026-11-07): QEMU VM (snapshot), Hyprland 0.57.1, foot, release build made in the
  guest, 12 generated 3840x2160 jpgs + 3 png + 3 small jpg in `~/Pictures/Wallpapers`. Floating
  foot 990x660 px = 80x24. Evidence images are in `tests/manual/s13/`.
  - PASS acceptance: `--version` prints `ln-hub 0.1.0`, rc 0; `--config /nonexistent.toml` stderr
    `ln-hub: config /nonexistent.toml: No such file or directory (os error 2)`, rc 2;
    modules=["Audio"] stderr `ln-hub: config ...: unknown module: Audio`, rc 2.
  - 1 PASS startup, execve to the end of the first frame (two writes, 1023 B + 482 B), in foot, 5
    runs: 3.9, 2.4, 1.7, 1.5, 0.9 ms (cold first run 3.9). Measured with
    `strace --seccomp-bpf -f -tt -e trace=execve,write`. Includes exec, config load, discover,
    terminal init and the first `draw`. Excludes foot's own startup and the picker query, which is
    sent after the first frame. In a terminal that does not answer: 2.0 ms (see 9).
  - 2 PASS sixel visible in foot: `tests/manual/s13/02-sixel-visible.png`.
  - 3 PASS sixel gone on Monitors, `tests/manual/s13/03b-sixel-gone-on-monitors.png`; redrawn on
    return, `tests/manual/s13/03c-sixel-redraw-after-return.png`; redrawn after `resizeactive`
    larger, `tests/manual/s13/03d-sixel-after-resize-larger.png`, smaller,
    `tests/manual/s13/03e-sixel-after-resize-smaller.png`, and `togglefloating` (tiled),
    `tests/manual/s13/03f-sixel-after-tile.png`. Before: `tests/manual/s13/03a-sixel-on-image.png`.
  - 4 PASS j/k bursts (11x j and 11x k at 20 ms, 17x j at 5 ms) over the 3840x2160 jpgs: stdout
    gaps stay 24-43 ms while keys arrive, 1-3 sixel writes (~170-200 KB each) per burst, final
    image correct: `tests/manual/s13/04-jk-many-large-images.png`.
  - 5 PASS apply wallpaper: waybar PID 7268 -> 9103 (new within 1.5 s); j/k handled during the
    job; `Applying wp_small_1.jpg…` in `tests/manual/s13/05b-applying.png` and
    `tests/manual/s13/05c-mid-apply.png`; then `Applied wp_small_1.jpg` and `●` moved from
    `videos/frieren_m…` to `wp_small_1.jpg` in `tests/manual/s13/05d-applied.png`. Before:
    `tests/manual/s13/05a-before-apply.png`.
  - 6 PASS S10: Enter while the job ran showed nothing and started no second job (`jobs.log`
    holds one `Using image wp_small_1.jpg` run), j still moved the cursor:
    `tests/manual/s13/06a-applying-enter-silent-j-moved.png`. The status stays after moving:
    `tests/manual/s13/06b-status-persists-after-moving.png`.
  - 7 PASS, fits at 80, truncates below; note reworded: at 80 columns the help line with quit key
    `q` is exactly 80 characters and shows in full, `tests/manual/s13/07-help-80col.png`. At 70
    columns it ends at `?`, `tests/manual/s13/07b-help-70col-truncates.png`.
  - 8 NEEDS USER CONFIRMATION: second output via `hyprctl output create headless`, 2 rows. At 80
    columns the items column is 19 wide and cuts the S9 rows `<name>  <w>x<h>@<hz>  on|off`
    to `Virtual-1  1268x13` and `HEADLESS-1  1920x1`; the `@hz` and `on/off` state is lost, and
    the preview pane is empty. `tests/manual/s13/08-monitors-80col.png`. S9 pins the row text
    but not its width, so whether this is acceptable is a design call.
  - 9 FAIL (bug): terminal that never answers the picker query (python pty, 24x80, no sixel
    support). First frame is drawn at 2.0 ms, then the UI is frozen for about 2 s (picker
    query timeout, `Picker::from_query_stdio`), then falls back to halfblocks. It does not hang
    for good, but the freeze eats keys and the first key after it. `q` sent at 0.2 s, 1.0 s and 2.2 s were all lost; `q` at 3.0 s was lost and
    `q` at 4.0 s exited rc 0. After the window, keys work and the preview renders as halfblocks
    (output has 970 `▀` / 163 `▄`, no sixel `ESC P`, one kitty query `ESC _G`; `x`, `l`, `j` at 2.6 /
    3.0 / 3.5 s, `q` at 5.5 s exited rc 0). Cause: the picker query runs on the main thread
    before the loop and leaves a stdin reader thread that takes the next key. Not run with
    `ssh -tt`.

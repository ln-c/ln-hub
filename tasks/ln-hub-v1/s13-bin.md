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
  - 8 NEEDS USER CONFIRMATION (superseded: S15 popup, re-run item 2 below): second output via `hyprctl output create headless`, 2 rows. At 80
    columns the items column is 19 wide and cuts the S9 rows `<name>  <w>x<h>@<hz>  on|off`
    to `Virtual-1  1268x13` and `HEADLESS-1  1920x1`; the `@hz` and `on/off` state is lost, and
    the preview pane is empty. `tests/manual/s13/08-monitors-80col.png`. S9 pins the row text
    but not its width, so whether this is acceptable is a design call.
  - 9 FAIL (bug; superseded: S14 fix, re-run item 1 below): terminal that never answers the picker query (python pty, 24x80, no sixel
    support). First frame is drawn at 2.0 ms, then the UI is frozen for about 2 s (picker
    query timeout, `Picker::from_query_stdio`), then falls back to halfblocks. It does not hang
    for good, but the freeze eats keys and the first key after it. `q` sent at 0.2 s, 1.0 s and 2.2 s were all lost; `q` at 3.0 s was lost and
    `q` at 4.0 s exited rc 0. After the window, keys work and the preview renders as halfblocks
    (output has 970 `▀` / 163 `▄`, no sixel `ESC P`, one kitty query `ESC _G`; `x`, `l`, `j` at 2.6 /
    3.0 / 3.5 s, `q` at 5.5 s exited rc 0). Cause: the picker query runs on the main thread
    before the loop and leaves a stdin reader thread that takes the next key. Not run with
    `ssh -tt`.

- Manual VM re-run (2026-11-07, after S14/S15): HEAD 94c772f (release build made in the guest from a
  tar copy sent over ssh; nothing was put under `~/git/dotfiles`). Fresh snapshot VM, Hyprland
  0.56.2 (QEMU virtio display, one output `Virtual-1` 1280x800), foot 1.28.0, floating
  foot 990x660 px = 24x80 (checked with `stty size`). Same wallpaper set as the first run (12 big
  jpg, 3 png, 3 small jpg). `ln-monitorctl` was taken from the mounted dotfiles repo
  (`PATH=~/git/dotfiles/.local/bin:$PATH`, read only), because the guest image's own copy
  predates the S0 fix and fails with two outputs. The silent terminal is a small python pty proxy
  (`/tmp/silentproxy.py` in the guest) run inside foot: it strips DA1, kitty, `CSI 16t` queries
  from ln-hub's output so foot never answers, and forwards keys; it timestamps events and can
  inject keys. Screenshots: `tests/manual/s14/` (query checks, regression) and
  `tests/manual/s15/` (popup).
  - 1a PASS (with one documented limit), silent terminal: first frame at 3 ms, the loop keeps
    drawing every 50-100 ms through the query window (no output gap above ~100 ms), the window
    ends at 255 ms, preview falls back to halfblocks
    (`tests/manual/s14/00-silent-baseline-halfblocks.png`). Seven keys injected from 300 ms on
    (`l k k k j k k`) were all handled, cursor on `wp_png_3.jpg` as expected, halfblocks preview:
    `tests/manual/s14/01b-silent-keys-after-deadline-all-handled.png`. `j` at 300 ms then `q` at
    400 ms: child exit 0 at 418 ms, first `q` press (log; `01c-silent-q-quits-first-press.png`
    is the shell after exit). Real typing with `wtype l k k k j k k` right after the window
    appeared (first key reached ln-hub at 395 ms, i.e. after the 250 ms window): all keys
    handled, same cursor, halfblocks:
    `tests/manual/s14/01d-silent-wtype-right-after-launch.png`. Limit: keys that arrive inside
    the 250 ms window of a silent terminal are dropped, not replayed (S14 decision, key bytes in
    the window are discarded). Six keys injected at 20-124 ms were all lost, only the `j` at 600 ms
    moved the cursor (Monitors selected): `tests/manual/s14/01a-silent-keys-inside-250ms-window-dropped.png`.
    The old failure (2 s freeze, `q` needing a 4 s wait, orphan reader eating a key) is gone.
  - 1b PASS, foot: sixel detected (foot answers `CSI 6;26;12t` + DA1 with attribute 4 within
    3 ms) and the preview is real sixel (smooth, not blocks):
    `tests/manual/s14/02a-foot-sixel-after-quick-keys.png` (compare
    `tests/manual/s13/02-sixel-visible.png`). `wtype l k k k j k k` right after launch: all
    handled, cursor on `wp_png_3.png`. Probe with one `j` injected at 10, 60, 120, 200, 300 ms after
    first output (pty proxy forwarding foot's answers): the 10 ms key was lost, 60 ms and later
    were handled, so in foot the drop window is well under 60 ms (a human cannot hit it).
  - 2 PASS S15 popup at 80 columns, one enabled monitor: column rows still truncate
    (`Virtual-1  1280x80`, `tests/manual/s15/01-monitors-80col-rows-truncated.png`). `Enter` opens
    a centred bordered popup with the full row `Virtual-1  1280x800@75  on` and
    `Refusing to disable the last enabled monitor`:
    `tests/manual/s15/02-popup-refusal-single-monitor.png`. Second `Enter` is silent, nothing
    spawned: `03-popup-refusal-enter-silent.png`. `Esc` closes, no change:
    `04-esc-closed-no-change.png`. `Enter` then `q` closes the popup and ln-hub stays running
    (pgrep), `05-q-closes-popup-app-alive.png`; `q` again quits, `06-q-quits-after-popup-closed.png`.
  - 2b PASS real toggle with a second output (`hyprctl output create headless` ->
    `HEADLESS-1` 1920x1080): rows at 80 columns, `07-two-monitors-rows-80col.png`; `Enter` on
    `HEADLESS-1` opens the popup `HEADLESS-1  1920x1080@60  on` / `HEADLESS-1 off`,
    `08-popup-full-row-headless-off.png`; `Esc` closes, `hyprctl monitors all` still
    `disabled: false` for both, `09-esc-closed-no-spawn.png`; `Enter`, `Enter`: after 0.45 s
    `Applied HEADLESS-1 off` and `hyprctl` shows `HEADLESS-1 disabled: true`
    (`10-confirm-applying.png` and `11-after-confirm-applied.png` both caught only the
    result: the job is too fast for a screenshot). Then the popup on the disabled row shows
    `HEADLESS-1  1920x1080@60  off` / `HEADLESS-1 on`, `12-popup-disabled-monitor-switch-on.png`;
    confirm: `Applying HEADLESS-1 on…` caught in `13-confirm-on-applying.png`, then
    `Applied HEADLESS-1 on` with hyprctl `disabled: false` in `14-confirm-on-applied.png`.
    Not run: the "job running" popup (covered by the S15 tests; the toggle job is too short to
    open a popup during it).
  - 3 regression spot-check, nothing changed vs the first run: help line at 80 columns shows in
    full, ends `q quit` (`tests/manual/s14/03a-reg-help-80col.png`); sixel on image
    (`03b-reg-sixel-on-image.png`), gone on Monitors (`03c-reg-sixel-gone-on-monitors.png`),
    redrawn on return (`03d-reg-sixel-redrawn-after-return.png`) and after `resize` larger
    (`03e-reg-sixel-after-resize-larger.png`), smaller (`03f-reg-sixel-after-resize-smaller.png`)
    and tiling (`03g-reg-sixel-after-tile.png`); apply wallpaper: waybar PID 3053 -> 4582,
    `Applying wp_small_1.jpg…` (`04b-reg-applying.png`), `Applied wp_small_1.jpg` with the `●`
    moved and the palette recoloured (`04d-reg-apply-applied.png`; before: `04a-reg-apply-before.png`).
    Observation (not a regression check, not compared with s13): with focus in Items and the
    help line open, the right-aligned `Font: ...` status overprints the end of the help line
    (see 03b, 03e-03g).
  - Environment limits: software-rendered virtual display, one real output plus one headless
    output, Hyprland 0.56.2 (first run had 0.57.1), the many-large-images j/k burst (old item 4) was not repeated,
    no `ssh -tt` run.
  - Guest `ln-monitorctl` (image copy, Oct 9) fails with two outputs: its loop exits 1 on the
    first non-matching monitor, so every toggle fails; the repo's fixed script (S0) works.
    Rebuild the VM base image or run the toggle with the repo's `.local/bin` first in `PATH`.

# S16 init status — VM check (2026-11-07)

Binary: `ln-hub 0.1.0`, release build of commit 13e69fe (branch feat/s16-s18-polish, clean tree).
Built on the host with `cargo build --release` (not run there) and sent to the guest with `scp`
as `/tmp/ln-hub`. Earlier VM runs built in the guest; this one did not.

## VM
- `.agents/skills/arch-vm-tester/scripts/run.sh` in `/home/ln/git/dotfiles`:
  QEMU/KVM, `-snapshot` (no `--no-snapshot`), ssh port 2222. Guest kernel 7.2.7-arch1,
  Hyprland 0.56.2, foot 1.28.0, one output `Virtual-1` 1280x800. Stopped with `stop.sh` at the end.
- Packages added inside the snapshot only: `wtype`, `strace` (pacman), `pyte` (pip --user).
- Hyprland was started from ssh (`/tmp/starth.sh`); the foot windows were floating 990x660 px =
  24x80 for these checks (`hyprctl dispatch hl.dsp.window.float/resize`).
- Generated wallpapers in `~/Pictures/Wallpapers` (e.g. `wp_small_1.jpg`) plus the image's
  own `frieren_memories_live` jpg/mp4. Font summary in the guest: `Font: Terminess Nerd Font Propo,
  size 15` (40 cells).
- Harness: `tools/proxy.py` is a transparent pty proxy that runs inside foot and spawns ln-hub in
  a pty. It forwards both ways (foot really answers the picker query), feeds the output into a
  `pyte` screen, timestamps events (`Initializing…` seen, `Ready` seen, `Ready` gone), and runs
  scheduled actions: `grim` screenshot, screen/row dump, `wtype` keys, direct key injection into
  the pty. `--silent` strips the DA1/kitty/CSI-t queries so foot never answers (the silent
  terminal of S14). `tools/run.sh` opens the foot window, `tools/vm.sh` is the host-side driver.
  Timestamps are ms since the proxy spawned ln-hub. The raw byte logs contain sixel payload and
  are not committed (only `s16-timeline/output-init-ready-lines.txt`).

## Results

| check | result |
|---|---|
| a. `Initializing…` while the query runs | PASS |
| b. `Ready`, cleared after ~1 s or by the first key | PASS |
| c. Keys right after launch not lost, nothing freezes | PASS (one documented limit, see c) |

### a. `Initializing…` (PASS)
- In foot (the terminal answers within ~3 ms): first frame with `Initializing…` in the status row
  at 4-9 ms after spawn, foot's answer arrives at 5-10 ms, `Ready` replaces it at 52-53 ms
  (104 ms once `wtype` was loading the CPU). So `Initializing…` is visible for ~45-50 ms. Timeline:
  `s16-timeline/timeline.log`; status row text at the three moments: `s16-timeline/rows.txt`;
  the bytes: `s16-timeline/output-init-ready-lines.txt`.
- A screenshot taken at that moment caught it: `s16-timeline/a-init-foot.png` (status row
  `Initializing…`).
- Silent terminal (never answers): `Initializing…` for ~250 ms (first frame 5.7 ms, `Ready` at 252.8
  ms, the S14 deadline). Visible in `s16-silent/a-init-silent.png` (+ `.txt` screen dump).

### b. `Ready` clears (PASS)
- No key: `Ready` is shown, then the row is blank 1003-1004 ms after it appeared (3 runs, e.g.
  `s16-timeline/timeline.log`: Ready 52.7 ms, cleared 1055.9 ms). Shots: `b-ready-foot.png`,
  `c-cleared-foot.png`.
- First key clears it at once and is still handled (key injected 200 ms after `Ready`):
  - `j`: `Ready` gone in the same event (204 ms after it appeared) and the module cursor moved to
    Monitors: `s16-key-j/after-j.png`, rows in `s16-key-j/rows.txt`.
  - `?`: `Ready` gone and the help line shows in full: `s16-key-help/after-help.png`,
    `rows.txt`.
  - `q`: process exits status 0 on that first press, 2 ms later (`s16-key-q/timeline.log`).

### c. Keys right after launch (PASS, with the documented drop window)
- `wtype ljj` started 10 ms after `Ready` (`s16-wtype-after-ready`): `l`, `j`, `j` reached ln-hub
  at 70/83/87 ms, all handled (Items focused, cursor moved to the last item, first key cleared
  `Ready`): `after-ljj.png`. No freeze; `q` 1.3 s later exits 0.
- `wtype ljj` started at spawn time, i.e. while `Initializing…` is up (`s16-wtype-start`): the keys
  reached the pty at 33-42 ms, before `Ready` (104 ms in this run). They were dropped, `Ready` was
  still showing 900 ms later and cleared only by its timer (1003 ms), focus unchanged
  (`after-ljj.txt`, `after-ljj.png`). This is the S14/S16 design (keys are not read and are
  dropped while the query runs; that is why the status row says `Initializing…`). With foot
  the window is ~50 ms, a human cannot hit it. Not counted as a failure.
- Silent terminal (`s16-silent`): `j` injected 20 ms after the first frame (inside the 250 ms
  window) was dropped (Theme still selected, `Ready` still showing at ready+100 ms:
  `b-ready-silent-key-in-window-dropped.png`); `j` 150 ms after `Ready` was handled (Monitors
  selected, `Ready` cleared): `c-late-j-handled.png`. `q` afterwards exits 0.
- Nothing stalled in any run: every injected key after `Ready` was handled within one frame and
  every `q` exited the process 2-4 ms after it was written.

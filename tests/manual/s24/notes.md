# S24 VM check: S0 bats, S22 responsiveness and timing (2026-11-08)

Branch `feat/s19-s24-followups`, code under test at HEAD `a742501` (S22 + S23). No code
change in this slice.

## PASS lines

- PASS S24-boot: the VM was booted with `arch-vm-tester` `HEADLESS=1 run.sh`, whose QEMU command
  line has `-snapshot` (see `boot.txt`). The base `disk.qcow2` has the same size and mtime before
  boot and after shutdown (2498494464 bytes, 2026-10-10 09:50:40.819608134 +0700). In the guest,
  `cp -f ~/git/dotfiles/.local/bin/ln-* ~/.local/bin/` copied the scripts from the 9p share.
  Afterwards the guest `ln-monitorctl` sha256 is `3062dfbf…0867025`, the same as the host copy.
  Before the copy it was `dfaae142…279cc0`, the stale pre-fix copy. All six `ln-*` scripts match
  the host.
- PASS S24-bats: in the guest, `cd ~/git/dotfiles && bats --tap tests/ln-monitorctl.bats` gave
  `1..14`, all 14 `ok`, exit 0 (Bats 1.14.0, guest kernel 7.2.7-arch1-1). The output is in
  `bats.tap`. This is the same file the spec names as `bats ~/git/dotfiles/tests/ln-monitorctl.bats`.
- PASS S24-s0: S0 is flipped to `green` in two places, the design doc's slice table and the
  front matter of `docs/superpowers/plans/archive/ln-hub-v1/s00-monitorctl-fix.md`. Its
  `Unverified (2026-11-07): …` line is replaced by a one-line verified note.
- PASS S24-responsive: the release build of `a742501` ran in foot 1.28.0, tiled, font size 9,
  176x43 cells, with `LN_HUB_TIMING` set. `l` was sent, then a timed burst while an encode was
  pending (the first hover of a fresh 6000x4000 JPEG). Every key showed its effect 8–21 ms after
  it was sent, against a ~100 ms budget. Three identical runs were made and no key went
  unresolved. `timeline.log` and `rows.txt` come from run 1; runs 2 and 3 are in `burst-2/` and
  `burst-3/`.

  | run | `l` | `?` (in encode window 1) | `j` (in encode window 1) | `k` (in encode window 2) |
  |---|---|---|---|---|
  | 1 (`timeline.log`) | 20.7 ms | 9.0 ms | 11.1 ms | 11.7 ms |
  | 2 (`burst-2/`) | 11.8 ms | 9.5 ms | 9.1 ms | 9.7 ms |
  | 3 (`burst-3/`) | 17.8 ms | 11.6 ms | 17.0 ms | 8.4 ms |

  Rows changed: `?` changes only row 42 (the help line), `j` and `k` change rows 1–2 (the item
  cursor), and `l` changes rows 1–4 and row 42.
- PASS S24-timing: `timing.log` is the `LN_HUB_TIMING` file from run 1. The release encode and
  draw times, in ms, were:

  | source | encode (102x30 cells) | draw |
  |---|---|---|
  | run 1 (`timing.log`) | 47.9, 65.7 | 36.7 |
  | run 2 (`burst-2/timing.log`) | 37.6, 37.6 | 32.4 |
  | run 3 (`burst-3/timing.log`) | 70.9, 46.7 | 32.5 |
  | shots (`shots/timing.log`) | 63.3 | 17.9 |
  | bare foot, no proxy (`timing-bare.log`) | 55.7, 51.2, 35.7 | 15.7, 13.6, 8.7 |

  The spec's baseline is ~20 ms cold encode in release and ~940 ms in debug. The guest encodes in
  ~36–71 ms, which is 2–3.5x the release baseline and far from the debug figure. The guest is a
  2-vCPU KVM VM with a 6000x4000 source, so this is in range for release. **The encode dominates
  the draw** in every run: 8.7–36.7 ms draw against 35.7–70.9 ms encode. So no open question about
  terminal-write cost is added. The custom-encoder open question in the spec ("decide after the
  S24 timing shows whether the encode or the terminal write dominates") now has its answer: the
  encode dominates, at ~40–70 ms per first hover in release in the VM.
- PASS S24-evidence: this directory holds:
  - Text dumps: `rows.txt`, `timeline.log`, `timing.log`, `bats.tap`, `boot.txt`,
    `timing-bare.log`, `final.txt`, `shots/*.txt`, and `burst-2/` and `burst-3/`.
  - PNGs, committed as Git LFS pointers: `final.png`, `shots/pending-decode.png`,
    `shots/pending-encode.png`, `shots/drawn.png`, `shots/bare-pending-150ms.png`,
    `shots/bare-drawn-3s.png`.
  - The tools, under `tools/`.

  **By eye, while the image is pending**, the preview pane shows only the text lines
  `wp_large_1.jpg` / `18M` / `image`. The image area under them is blank: you see foot's
  translucent background. No stale picture and no partial image are visible. That is what S22
  expects. The screenshots:
  - `shots/pending-decode.png`: grim started ~25 ms after `l`, while the decode was in flight.
  - `shots/pending-encode.png`: grim started ~7 ms into encode window 1; the sixel arrived 47 ms
    later.
  - `shots/bare-pending-150ms.png`: no proxy, 150 ms after `l`.

  The image shows up afterwards in `shots/drawn.png`, `shots/bare-drawn-3s.png` and `final.png`:
  a coloured plasma picture under the meta lines. `final.png` also shows the help line, and the
  cursor back on `wp_large_1.jpg`. Caveat: grim's exact capture moment, relative to the app's
  frames, is not known to the millisecond. So this records "blank while the image is pending
  (decode and/or encode)", not a frame-exact claim about the encode alone.

## Procedure

Host tools are in `/home/ln/git/dotfiles/.agents/skills/arch-vm-tester/scripts`. Nothing in
`~/git/dotfiles` was edited; its `git status` stayed clean.

1. **Boot.** Record `stat` of `vm_data/disk.qcow2`, then run `HEADLESS=1 run.sh`. The default
   instance is the only VM, so no `INSTANCE_ID` was needed. Record the QEMU cmdline from
   `/proc/<pid>/cmdline`; it contains `-snapshot`.
2. **Scripts.** In the guest:
   `cp -f ~/git/dotfiles/.local/bin/ln-* ~/.local/bin/ && sha256sum ~/.local/bin/ln-*`.
   On the host: `sha256sum ~/git/dotfiles/.local/bin/ln-*`. The two lists are compared in
   `boot.txt`.
3. **Bats.** In the guest:
   `cd ~/git/dotfiles && bats --tap tests/ln-monitorctl.bats > /tmp/bats.tap; echo exit=$?`
   gave `exit=0`. The TAP was copied to `bats.tap`.
4. **Guest session.** This setup lives in the snapshot only:
   - Hyprland 0.56.2 was started from ssh on seatd (`tools/starth.sh`). Its output is
     `Virtual-1`, 1280x800.
   - `/tmp/vmenv.sh` exports `XDG_RUNTIME_DIR`, `WAYLAND_DISPLAY=wayland-1` and
     `HYPRLAND_INSTANCE_SIGNATURE`.
   - `pip install --user --break-system-packages pyte` was run.
   - `~/.config/ln-hub/config.toml` has `wallpaper_dir = "~/Pictures/s24"`.
   - Four fresh wallpapers were generated, `~/Pictures/s24/wp_large_{1..4}.jpg`, each
     6000x4000 and ~18 MB: `magick -seed $i -size 1500x1000 plasma:fractal -resize 6000x4000!
     -attenuate 0.6 +noise Gaussian -quality 92`.
   - The status row's font is `Terminess Nerd Font Propo, size 15`.
5. **Binary.** On the host at `a742501`, `cargo build --release`, then `scp` of
   `target/release/ln-hub` to the guest's `/tmp/ln-hub`. The sha256 is
   `41422d8c1fae59d3c2fa1d42e440ea06a5bada0920daf1d07f8867e55db5d46a` on both sides.
6. **Harness.** `tests/manual/s16/tools/vm.sh` and `run.sh` were reused unchanged. The host copied
   them, plus `tools/proxy.py`, to the guest's `/tmp`. `run.sh NAME tiled 9` opens foot with
   font size 9, which tiles to 176x43 cells (the proxy logs `winsize=43x176`). foot runs
   `proxy.py`, and `proxy.py` runs ln-hub on a pty.
7. **Burst runs.** `tools/burst.sh burst1` (and `burst2`, `burst3`) does the following:
   - Sends `l` 200 ms after the `Ready` status clears. This puts the cursor on
     `wp_large_1.jpg` and starts its decode.
   - When encode window 1 opens (see below), sends `?` and, 20 ms later, `j`. `j` moves the
     hover to `wp_large_2.jpg` while `wp_large_1`'s encode is still in flight.
   - When encode window 2 opens (`wp_large_2`'s encode in flight), sends `k`, back to
     `wp_large_1`.
   - Dumps the screen and takes grim `final` 1.5 s later, then quits.
8. **Screenshots.** `tools/shots.sh shots` takes grim shots and dumps at three points: just after
   `l`, at the start of encode window 1, and ~2.5 s after `l`.
9. **Bare cross-check.** `tools/bare.sh bareN` runs in the guest. ln-hub runs directly in foot,
   with no proxy and `LN_HUB_TIMING` set. `l` is sent with Hyprland
   `hl.dsp.send_shortcut({..., window="title:lnhub-N"})`, and grim shots are taken at 150 ms and
   3 s. This was done three times; the times are in `timing-bare.log`.
10. **Teardown.** `stop.sh`. Afterwards no qemu process is left, and the `disk.qcow2` stat is
    unchanged.

### How "an encode is pending" is detected

`tools/proxy.py` is a copy of `tests/manual/s16/tools/proxy.py` with these changes:

- **Encode windows.** While an encode is in flight, S22's UI loop polls every 10 ms instead of
  100 ms, so its output chunks come <25 ms apart with no key behind them. The proxy opens
  `FAST window N` when two output chunks arrive <25 ms apart and no key was sent in the prior
  40 ms. The window becomes the event trigger `fastN`. It closes on a sixel chunk or a >60 ms
  gap. Sixel payload continuation chunks are ignored.
- **The `key` action.** It snapshots the screen (characters plus fg/bg/bold/reverse) and logs
  `EFFECT key 'x' +N ms … rows changed [...]` at the first visible change. Keys still without an
  effect at exit are logged as `UNRESOLVED`; there were none.
- **Linear DCS stripping.** The S16 proxy re-scanned the whole growing buffer for DCS on every
  chunk, and fed every 4 KiB sixel chunk through pyte. With S16's proxy, two calibration runs
  logged draw times of 634 and 722 ms. That was the proxy throttling the pty, not ln-hub. The
  DCS stripping is now a linear state machine, and pyte is skipped for sixel-payload chunks.
  After this change, draw is 15–37 ms, the same order as the bare foot runs without the proxy
  (8.7–15.7 ms). The calibration runs are not committed.

The proxy reads keys and effects at the pty. So the latency is key-in to frame-out of ln-hub
itself; foot's own paint adds at most about one display frame. Keys go through the pty, not foot's
keyboard path.

Each key landed inside an encode window, checked against the window bounds in `timeline.log`:
- Run 1: window 1 was 1686.6–1735.8 ms, with `?` at 1693.4 and `j` at 1713.5. Window 2 was
  2149.7–2158.2 ms, with `k` at 2158.1.
- Runs 2 and 3: same layout.

In run 1, `k`'s frame drew the already encoded `wp_large_1` sixel. Only two `encode` lines exist,
one per image, so it was not re-encoded. `wp_large_2`'s encode completed and was not drawn,
because the hover had moved.

## Deviations

- **Keys were not sent with `wtype`.** `sudo pacman -S wtype` in the guest was refused by the
  agent's permission policy. In the burst runs the proxy injects keys into the pty. In the bare
  runs, Hyprland `send_shortcut` with a `window=` selector sends them.
- **The burst runs used a fixed proxy.** They used `tools/proxy.py`, not the unchanged S16 proxy
  (see above). `vm.sh` and `run.sh` are S16's, unchanged; their copies in `/tmp` were identical.
- **Bats was run with `--tap` from the dotfiles root.** This is the same suite file the spec
  names.
- **No open question was added to the spec.** The draw does not dominate. The S24 answer to the
  custom-encoder question is recorded here only, because the spec's open-question lines are not
  owned by this slice.

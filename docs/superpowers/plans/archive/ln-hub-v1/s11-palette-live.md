---
spec: ../../../specs/2026-11-01-ln-hub-v1-design.md
slice_id: S11-palette-live
status: green
---

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

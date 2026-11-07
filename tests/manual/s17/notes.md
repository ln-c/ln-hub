# S17 help/status vs `Font:` overlap — VM check (2026-11-07)

Binary: `ln-hub 0.1.0`, release build of commit 13e69fe (branch feat/s16-s18-polish, clean tree),
built on the host, sent with `scp` (not run on the host). VM, packages and the pty proxy harness
are described in `../s16/notes.md` (same session, same snapshot VM: QEMU `-snapshot`, Hyprland
0.56.2, foot 1.28.0). The harness code is in `../s16/tools/`.

Setup: Theme module, Items focused (`l`), guest font `F` = `Font: Terminess Nerd Font Propo, size 15`
(40 cells, not the 29-cell font of the unit tests). `H` = the help line (80 cells). Window widths
come from `foot` geometry; the column count is read from the pty size in each `timeline.log` and
the screen dumps. Window sizes: 176 cols (tiled, font size 9), 121 and 120 (868/876 px float,
font size 9), 80 (990 px), 44 (560 px), 39 (500 px), 28 (372 px), all at 24 rows except the first
three. The bottom row text is taken from a `pyte` screen fed with ln-hub's output, so it is
exactly what foot drew, and the screenshots agree where looked at.

`verify.py` re-computes the expected row from the rule (F whole and right-aligned iff
`len(left) + 1 + 40 <= width`, else no F cell at all; empty left: F right-aligned, clipped by the
row edge below 40 cols) and compares every captured row: `verify-s17.txt`, ALL PASS (55 rows).

## Results

| check | result |
|---|---|
| a. `?` help open: never overprinted by F; F whole with >= 1 blank or omitted | PASS |
| b. other left text: same rule (filter prompt, job status, long status) | PASS (startup/Ready: not reachable, see b) |
| c. empty left: F right-aligned, clipped by the row edge when narrow | PASS |
| S18 note: sixel preview still draws on `l` | PASS |

### a. Help (`help-<cols>/c-help.txt` + `c-help.png`)
| cols | last row | expected |
|---|---|---|
| 176 | `H`, 56 blanks, F (right-aligned) | both |
| 121 (= 80+1+40) | `H`, exactly one blank, `F` | both, one blank |
| 120 | `H` whole, no `Font` | F omitted whole |
| 80 | `H` whole, ends `q quit` | no F |
| 44 | first 44 cells of `H` (`... / fi`), no `Font` | clipped by the edge only |
| 39 | first 39 cells of `H`, no `Font` | same |
| 28 | first 28 cells of `H`, no `Font` | same |
No row anywhere holds characters of both `H` and a cut piece of F. Screenshots: `help-80/c-help.png`,
`help-39/c-help.png`, `help-121/c-help.png`, `help-120/c-help.png`, `help-44`, `help-28`, `help-176`.
Incidental: at 176x43 the UI was busy for a few hundred ms after `l` (sixel encode at a big area);
the `?` was applied before the +800 ms dump (the first attempt dumped 300 ms after it and still saw
the old row, rerun with 800 ms). Not an S17 matter.

### b. Other left text
- Filter prompt at 80 cols, typing `a` after `/` (`pr80/rows.txt`): prompt `Filter: ` + 31 chars
  (39 cells) keeps F with one blank; with 32 chars (40 cells) F is omitted whole and the prompt
  is whole. This is exactly `39 + 1 + 40 = 80`. `pr80/p31.png` (F shown) and `pr80/p32.png`
  (F gone). A wider prompt (49 chars, 57 cells) is whole, no F.
- Filter prompt at 44 cols (`pr44/rows.txt`): `Filter:` never leaves room, F is absent for every length, the
  prompt is whole up to the row edge.
- Job status at 80 cols, `Applying wp_small_1.jpg…` (24 cells): F shown whole next to it for the
  whole job and after `Applied wp_small_1.jpg` (`jobs80/rows.txt`, `job-status.png`).
- Job status at 44 cols: same text, F omitted (`jobs44/rows.txt`).
- Longer status at 80 cols, `Applying videos/frieren_memories_live.mp4…` (42 cells; 42+1+40 > 80):
  F omitted whole, then `Applied videos/...` likewise (`job80-video/rows.txt`, `job-status.png`).
- Long error status at 80 cols, `ln-themectl not found: No such file or directory (os error 2)`
  (61 cells; the first attempt, `ln-themectl` was not on PATH): shown whole, no F
  (`job80-notfound/rows.txt`).
- Startup text (`Initializing…`/`Ready`) next to F: not reachable in the binary. F is drawn only
  with Items focused, which needs a key, and the first key clears `Ready` (and keys during
  `Initializing…` are dropped). The Modules-focused startup rows show no F at any width
  (`../s16/s16-timeline/rows.txt`). Rule for it is covered by the unit tests.

### c. Empty left (`help-<cols>/b-items-left-empty.txt`)
80: 40 blanks then F, right-aligned (last cell = last column). 44: 4 blanks then F. 39: the first
39 cells of F (`Font: Terminess Nerd Font Propo, size 1`) = clipped by the row edge. 28: the first
28 cells (`Font: Terminess Nerd Font Pr`). 120/121/176: right-aligned. With the Modules column
focused the row is blank at every width (`a-modules-left-empty.txt`).

### S18 in passing (`s18-sixel/`)
`l` on `wp_small_1.jpg` (80x24, foot): smooth sixel gradient preview below the preview text,
`sixel-on-l.png`; after `j` onto the next item the preview moves on, `sixel-after-j.png`. Same
rendering as `../s14/02a-foot-sixel-after-quick-keys.png`. Also seen in `../s16/s16-wtype-after-ready/after-ljj.png`
(video thumbnail).

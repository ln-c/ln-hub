---
spec: ../../specs/ln-hub-v1.md
slice_id: S17-help-status-overlap
status: green
---

### Slice 17
- slice_id: S17-help-status-overlap
- behavior: The right-aligned `Font: ...` summary never overprints the left side of the status
  row. The left text (help line, job status, prompt) has priority; the font summary is drawn
  only when it fits next to it. (From the manual VM re-run of S14: with focus in Items and the
  help line open at 80 columns, `tests/manual/s14/03b-reg-sixel-on-image.png` and the `03e` to
  `03g` shots show the end of the help line replaced by `Font: ...`.)
- seam: `App::render` with `TestBackend` at 80, 70 and 120 columns.
- test_paths: [tests/s17_help_status_overlap.rs]
- owned_paths: [src/app.rs]
- root cause: `App::render_status` splits the status row into `Fill(1)` for the left text and
  `Length(font_width)` for the font summary. The font summary always keeps its columns, so the
  left text is cut at `width - font_width`. The help line with the default quit key is exactly
  80 characters (S13), so at 80 columns every item-focused Theme view loses the end of it
  (`... ? help  q quit` becomes `... Font: Name, size 12`). Long statuses such as
  `Apply failed: <reason>` lose their end the same way.
- acceptance_criteria:
  - Setup: Theme module, Items focused, a font in `env.lua` (`Font: JetBrains Mono, size 12`,
    29 cells), the default quit key `q`. Let `H` be the help text and `F` the font summary.
  - 80 columns, `?` pressed: the last row is exactly `H` (80 cells, ending `q quit`;
    trailing blanks aside). `Font:`
    appears nowhere in that row.
  - 70 columns, `?` pressed: the last row is the first 70 cells of `H` and nothing else. `Font:`
    does not appear. The left text is clipped by the row edge only, as in the module-column
    view.
  - 120 columns, `?` pressed: the last row starts with the whole `H`, and `F` is whole and
    right-aligned (its last cell is the last column), with at least one blank cell between them.
  - Boundary: at width `len(H) + 1 + len(F)` (110) both show with exactly one blank cell
    between; at `len(H) + len(F)` (109) `F` is hidden and `H` is whole.
  - No status (nothing pressed) at 80 columns: `F` is shown right-aligned as before (regression).
  - A short status still lets the font show: `Enter` on a wallpaper → `Applying wp.png…` and
    `F` both whole at 80 columns. The rule is per frame: once the long text is replaced by a
    short one, `F` is back.
  - A long status hides `F`: at 80 columns a status of 70 cells in total (`Apply failed: ` plus
    a 56-char reason) is whole and `F` is absent; a status wider than the row (e.g. 84 cells)
    is clipped by the row edge only, and `F` is absent.
  - An open prompt takes priority too: `f` and typing a name wider than the room left for it
    shows the prompt text whole and no `F`; a short prompt text keeps `F`.
  - Module column focused (`F` is not shown there): the help line at 80 and 70 columns is as
    today (unchanged).
  - Never mixed: in no tested case does one row hold characters of both the left text and a
    cut-off part of `F`.
- Decisions:
  - Rule: the left text (open prompt, else status message, else startup text from S16) has
    priority. `F` is drawn only when `width(left) + 1 + width(F) <= width(row)`; otherwise it is
    omitted whole. `F` is never truncated or shown partly: half a font name is misleading, and
    the same summary is one `s`/`f` away. Rejected: truncating the left text with `…` (the help
    line is the one text that must stay whole) and shrinking `F` to `Font: Name`.
  - The rule covers every left text, not only the help line, since the root cause is the same
    layout. When the left text is empty, `F` is drawn as today (a width over the row is still
    clipped by the row).
  - The empty-left case is pinned by tests at widths 29 (exactly `F`), 20 (the leading 20 cells
    of `F`) and 30 (one blank, then `F`).
  - Implementation shape (the implementer may differ): measure the left text with the same
    `Line::width()` as today, compute `font_width` as 0 when it does not fit, keep the layout
    split. No change to `Font::summary` or the pane.
  - Only the status row changes. The help text and the keymap are unchanged. S8 tests that
    read the font summary next to a short status keep passing; if one shows both with a text
    wider than the room, it is a RED test to revisit, not to loosen.
  - Not in scope: wrapping the help line to two rows, a second status row, a width-adaptive
    short help, or moving `F` to the Items block title.

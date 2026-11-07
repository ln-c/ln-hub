---
spec: ../../specs/ln-hub-v1.md
slice_id: S12-filter
status: green
---

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
    `Apply` guard in `App`. Filtering while a job is in flight is pinned in s12_filter.rs.
  - `ThemePane::visible()` allocates and lowercases on every call; cache it if lists grow.
  - `Action` mixes app-level (Quit, Left/Right focus) and pane-level variants; fine with two
    panes.
  - `PromptKind` still switches in `line`/`command`/`handle_prompt_key`; extract a `TextInput`
    if editing grows.
  - Cosmetic: `MonitorsPane::unfocus` is an empty method kept for the exhaustive match;
    `toggle`'s `match m.disabled` reads worse than `if`/`else`; the Esc arm must clear before
    closing the prompt (add a comment); the two `if filtering { cursor = 0 }` copies and the
    two `Prompt::new(kind, String::new())` calls could be folded.
  - Untested: `.saturating_sub(1)` in the module-column `Down` (empty module list). `Esc` from
    a reopened filter prompt is pinned in s12_filter.rs.
  - `gg`/`G` (top/bottom) and `?` (help) from the default keymap: moved to S13.

---
spec: ../../../specs/2026-11-01-ln-hub-v1-design.md
slice_id: S10-monitor-toggle
status: green
---

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

---
spec: ../../specs/ln-hub-v1.md
slice_id: S15-monitor-popup
status: pending
---

### Slice 15
- slice_id: S15-monitor-popup
- behavior: `Enter` on a monitor opens a popup overlay with the full row and the switch it
  will perform; a second `Enter` confirms. Rows in the column stay truncated at 80 columns.
  (From manual VM run item 8.)
- seam: `App::handle_key` / `App::render` with `TestBackend` and the `FakeSystem` call log.
- test_paths: [tests/s15_monitor_popup.rs, tests/s10_toggle.rs]
- owned_paths: [src/**]
- acceptance_criteria:
  - The popup opens at every width (80 and 120 columns). `Enter` on `DP-1` (on, 1920x1080@60)
    → no spawn; the popup shows `DP-1  1920x1080@60  on` and `DP-1 off`.
  - `Enter` in the popup → `spawn("ln-monitorctl", ["DP-1","off"])`; the popup closes; the
    status reads `Applying DP-1 off…`, then `Applied DP-1 off` after `Done`, or
    `Apply failed: <reason>` after `Failed`, as in S10. A disabled monitor shows and spawns
    `on`.
  - `Esc` closes the popup with no spawn. The quit key closes it too and does not quit the
    app (`should_quit()` stays false). Quitting works again once it is closed.
  - Other keys (`j`, `k`, `h`, `l`, `/`, `g`, `?`) are swallowed while it is open: the cursor,
    the focus and the status do not change.
  - Refusal: only `eDP-1` on; `Enter` on `eDP-1` → the popup shows the full row and
    `Refusing to disable the last enabled monitor`. `Enter` in it does nothing (no spawn, no job
    in flight); `Esc` and the quit key close it.
  - A job is already running; `Enter` on a monitor → the popup shows `Applying…`. `Enter` in
    it is silent (no spawn); `Esc` closes it.
  - While the popup is open `App::preview_image()` is `None`, also after a `render`.
  - The S10 tests that toggled on the first `Enter` are rewritten to go through the popup
    (`Enter`, `Enter`) in RED. The refusal test now reads the popup text instead of the status.
- Decisions:
  - The popup replaces the S10 immediate toggle at all widths, not only when the row is
    truncated. Rows in the column are unchanged (S9 pins the text, not the width).
  - The confirm spawns exactly like the S10 toggle: same `Power` choice, same `Job` /
    `OnDone::ReloadMonitors`, same status wording. S10's "status not cleared" and "Enter while a
    job runs is silent" stay.
  - The popup is `App` state (a `Popup` value), not a pane prompt: it takes every key before
    the keymap, like an open prompt, but the quit key closes it instead of being text.
  - The popup shows the state at open time. A `tick` that finishes a job or reloads the list
    while it is open does not rewrite its text; the refusal / `Applying…` view is decided when
    it opens. Unpinned.
  - `preview_image()` is `None` while open because the bin paints the sixel after `app.render`,
    and it would cover the popup. Monitors already have no preview image, so the guard is
    cheap insurance (the field is only rewritten by `render`); it keeps the rule explicit for
    a later popup elsewhere.
  - Not in scope: a width-adaptive column, scrolling the row, or a popup for the Theme pane.

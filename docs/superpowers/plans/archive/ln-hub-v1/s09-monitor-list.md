---
spec: ../../../specs/2026-11-01-ln-hub-v1-design.md
slice_id: S9-monitor-list
status: green
---

### Slice 9
- slice_id: S9-monitor-list
- behavior: The Monitors module lists monitors from `hyprctl -j`.
- seam: `App` with the `FakeSystem` `run("hyprctl", ["monitors","all","-j"])` canned output.
- test_paths: [tests/s9_monitors.rs]
- owned_paths: [src/**]
- acceptance_criteria:
  - Canned JSON with `eDP-1` (2880x1800@120, `disabled:false`) and `DP-1` (2560x1440@60,
    `disabled:true`) → rows `eDP-1  2880x1800@120  on` and `DP-1  2560x1440@60  off`.
  - `run` returns an error → the items column shows `hyprctl unavailable`. No panic.
- review follow-up (pinned in tests/s9_monitors.rs):
  - Unparseable `hyprctl` output also shows `hyprctl unavailable`. This covers a non-zero
    exit, because `ProcSystem::run` returns `Ok` with empty stdout.
  - `hyprctl` runs only when `l` focuses Monitors, never at start-up or on Theme. Pressing `l`
    again re-queries.
  - `f`, `s` and Enter do nothing on Monitors.
  - Carried from S8: an open prompt survives `tick` finishing a job. The `Applied …` status
    shows after Esc. The font line is absent on Monitors.
- Carried notes: resolved, or moved to S10–S13 (audited 2026-11-07).

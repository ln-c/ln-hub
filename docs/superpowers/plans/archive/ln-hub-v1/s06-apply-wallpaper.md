---
spec: ../../../specs/2026-11-01-ln-hub-v1-design.md
slice_id: S6-apply-wallpaper
status: green
---

### Slice 6
- slice_id: S6-apply-wallpaper
- behavior: `Enter` applies the selected wallpaper as a background job and shows its progress.
- seam: `App::handle_key(Enter)`, `App::tick`, and the `FakeSystem` call log / job states.
- test_paths: [tests/s6_apply.rs]
- owned_paths: [src/**]
- acceptance_criteria:
  - `Enter` on `img2.png` → exactly one `spawn("ln-themectl", ["-w", "<abs>/img2.png"])`. The
    status bar shows `Applying img2.png…`.
  - A second `Enter` while the job is `Running` → no second spawn.
  - Job set to `Done`, then `tick()` → the status shows `Applied img2.png`.
  - `Failed("exit 1")`, then `tick()` → the status shows `Apply failed: exit 1`.
  - `spawn` returns an `io::Error` "not found" → the status shows
    `ln-themectl not found: not found`. No panic.
  - (Review follow-up, S5 carry-over) Job `Done`, then `tick()` → `current_wallpaper` is
    re-read, so `●` moves to the applied wallpaper and the cursor stays put. `Failed` → no
    re-read.
- Carried notes: resolved, or moved to S10–S13 (audited 2026-11-07).

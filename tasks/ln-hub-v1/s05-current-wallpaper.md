---
spec: ../../specs/ln-hub-v1.md
slice_id: S5-current-wallpaper
status: green
---

### Slice 5
- slice_id: S5-current-wallpaper
- behavior: The currently applied wallpaper is marked and preselected.
- seam: `App` render.
- test_paths: [tests/s5_current.rs]
- owned_paths: [src/**]
- acceptance_criteria:
  - `current_wallpaper` file contains `<dir>/img2.png\n` → the `img2.png` row contains `●` and is
    highlighted on entering Theme.
  - A missing file → no `●`, and the highlight is on the first item.
  - (Review follow-up) A `current_wallpaper` path not in the list (e.g. same file name outside
    the wallpaper dir) → no `●`, highlight on the first item.
- Carried notes: resolved, or moved to S10–S13 (audited 2026-11-07).

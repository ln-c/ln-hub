---
spec: ../../specs/ln-hub-v1.md
slice_id: S7-preview-meta
status: green
---

### Slice 7
- slice_id: S7-preview-meta
- behavior: The preview column shows details of the hovered wallpaper. The image itself is drawn
  only by the bin.
- seam: `App` render.
- test_paths: [tests/s7_preview.rs]
- owned_paths: [src/**]
- acceptance_criteria:
  - Hovering `img2.png` (2048 bytes) → the preview shows `img2.png` and `2.0K`.
  - Hovering `b.mp4` with `b.jpg` beside it → the preview shows `video` and `thumb: b.jpg`.
  - (Review follow-up) Sizes are `ls -h` style and round up: 10 → `10`, 1500 → `1.5K`,
    1025 → `1.1K`, 10240 → `10K`, 1048576 → `1.0M`. An image shows `image` and no
    `thumb:`. The preview is empty until items are focused, and also when the list is empty.
- Carried notes: resolved, or moved to S10–S13 (audited 2026-11-07).

---
spec: ../../specs/ln-hub-v1.md
slice_id: S4-wallpaper-list
status: green
---

### Slice 4
- slice_id: S4-wallpaper-list
- behavior: The Theme module lists wallpapers. Video thumbnails are hidden, and the list is
  sorted naturally.
- seam: `App` (focus Theme, `l`) → items column.
- test_paths: [tests/s4_wallpapers.rs]
- owned_paths: [src/**]
- acceptance_criteria:
  - Wallpaper dir with `img10.png`, `img2.png`, `b.mp4`, `b.jpg`, `notes.txt` and
    `images/c.jpg` → items render in the order `b.mp4`, `img2.png`, `img10.png`, `images/c.jpg`.
    `b.jpg` (thumb of `b.mp4`) and `notes.txt` are absent.
  - An empty or missing dir → the items column shows `No wallpapers in <dir>`. No panic.
  - (Deferred from S3) An `App` built from `Config::default_for(home)` lists the wallpapers in
    `<home>/Pictures/Wallpapers`.

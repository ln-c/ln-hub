---
spec: ../../specs/ln-hub-v1.md
slice_id: S3-config
status: green
---

### Slice 3
- slice_id: S3-config
- behavior: `config.toml` overrides default paths, module order/visibility and keys.
- seam: `Config::from_toml`, `Config::default_for`, then observed through `App`.
- test_paths: [tests/s3_config.rs]
- owned_paths: [src/**]
- acceptance_criteria:
  - `default_for("/h")`: an App built from it lists wallpapers from `/h/Pictures/Wallpapers`
    (verified with a temp home).
  - `modules = ["Monitors"]` → the rendered module column shows `Monitors` and not `Theme`.
  - `modules = ["Monitors","Theme"]` → `Monitors` is rendered above `Theme`.
  - `[keymap]\nquit = "x"` → `x` quits and `q` no longer quits.
  - `modules = ["Audio"]` → `Err(ConfigError::UnknownModule("Audio"))`.
  - `wallpaper_dir = "~/walls"` with home `/h` → resolves to `/h/walls`.

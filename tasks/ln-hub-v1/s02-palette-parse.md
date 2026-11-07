---
spec: ../../specs/ln-hub-v1.md
slice_id: S2-palette-parse
status: green
---

### Slice 2
- slice_id: S2-palette-parse
- behavior: The palette is parsed from wallust `colors.sh`.
- seam: `Palette::parse_colors_sh`, `Palette::get`
- test_paths: [tests/s2_palette.rs]
- owned_paths: [src/**]
- acceptance_criteria:
  - Input `background='#170000'\nforeground='#e0d0c0'\ncolor4='#aabbcc'\n` plus color0..15
    gives `get("color4") == Some(Color::Rgb(0xaa,0xbb,0xcc))`.
  - The same input with no `color4` line → `Err(PaletteError::Missing("color4"))`.
  - `color4='zzz'` → `Err(PaletteError::Invalid("color4"))`.
  - Comment lines and `wallpaper='...'` are ignored without error.

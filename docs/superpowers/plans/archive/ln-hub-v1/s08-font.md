---
spec: ../../../specs/2026-11-01-ln-hub-v1-design.md
slice_id: S8-font
status: green
---

### Slice 8
- slice_id: S8-font
- behavior: Shows the current font name and size, and sets them through `ln-themectl`.
- seam: `App` keys `f` / `s`, typed text, `Enter`, `Esc`.
- test_paths: [tests/s8_font.rs]
- owned_paths: [src/**]
- acceptance_criteria:
  - `env.lua` with `hl.env("FONT_NAME", "Terminess Nerd Font Propo")` and
    `hl.env("FONT_SIZE", "16")` → the Theme view shows `Terminess Nerd Font Propo` and `16`.
  - A missing `env.lua` → shows the defaults `Terminess Nerd Font Propo` / `16`.
  - `f`, typing `JetBrainsMono Nerd Font`, then `Enter` →
    `spawn("ln-themectl", ["-f", "JetBrainsMono Nerd Font"])`.
  - `s`, typing `18`, then `Enter` → `spawn("ln-themectl", ["-s", "18"])`.
  - `s`, typing `abc`, then `Enter` → no spawn, and the status shows `Font size must be 6-72`.
    The same happens for `5` and `73`.
  - `Esc` during input → no spawn, and the prompt closes.
  - (Review follow-up) Pinned:
    - A Done job re-reads env.lua; a Failed job does not.
    - Every key typed into an open prompt is text, including the quit key.
    - `f` / `s` work only with the Theme items focused.
    - `Enter` in a prompt waits (no spawn, prompt kept) while a job is in flight.
    - `Backspace` edits the prompt text.
    - Name and size are trimmed, and an empty name does nothing.
    - The prompts are labelled `Font name:` / `Font size:`.
    - Font jobs show `Applying font size 18…` / `Applied font size 18` (and the same for `font name`).
    - The font line is hidden until the items column is focused.
- Carried notes: resolved, or moved to S10–S13 (audited 2026-11-07).

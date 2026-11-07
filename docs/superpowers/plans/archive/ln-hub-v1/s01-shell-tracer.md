---
spec: ../../../specs/2026-11-01-ln-hub-v1-design.md
slice_id: S1-shell-tracer
status: green
---

### Slice 1
- slice_id: S1-shell-tracer
- behavior: The app renders three columns. The module column lists `Theme` and `Monitors`, and
  `q` quits.
- seam: `App::new(Config::default_for(tmp), FakeSystem)` → `render` on an 80x24 `TestBackend`;
  `handle_key('q')` → `should_quit()`.
- test_paths: [tests/s1_shell.rs, tests/common/mod.rs]
- owned_paths: [Cargo.toml, src/**]
- acceptance_criteria:
  - The rendered buffer contains `Theme` on a row above `Monitors`, both in the leftmost column.
  - `should_quit()` is `false` before and `true` after the key `q`.
  - `j` moves the module highlight from `Theme` to `Monitors`. A second `j` keeps it on
    `Monitors` (no wrap).

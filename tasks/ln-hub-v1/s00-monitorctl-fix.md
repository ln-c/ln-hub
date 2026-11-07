---
spec: ../../specs/ln-hub-v1.md
slice_id: S0-monitorctl-fix
status: unverified
---

### Slice 0 (dotfiles prerequisite)

Unverified (2026-11-07): bats tests not yet run; VM image's ~/.local/bin/ln-monitorctl predates the fix (seen in the S13 re-run).

- slice_id: S0-monitorctl-fix
- behavior: `ln-monitorctl <name> on|off` accepts any listed monitor, not only the first one, and
  exits 0 on success. `ini` exits 0. Comparisons are numeric.
- seam: CLI `ln-monitorctl <monitor> [on|off]` | `ln-monitorctl ini`, with `hyprctl` stubbed via
  bats-mock (external boundary).
- test_paths: [/home/ln/git/dotfiles/tests/ln-monitorctl.bats]
- owned_paths: [/home/ln/git/dotfiles/.local/bin/ln-monitorctl]
- acceptance_criteria:
  - Monitors `eDP-1`, `DP-1`; `ln-monitorctl DP-1 off` → hyprctl receives
    `eval hl.monitor({ output = "DP-1", disabled = true })`, exit 0.
  - Same monitors; `ln-monitorctl DP-1 on` → `... output = "DP-1", disabled = false ...`, exit 0.
  - `ln-monitorctl HDMI-A-9 off` → stderr contains `Invalid monitor`, exit 1, no `eval` call.
  - `ln-monitorctl ini` with 2 monitors → disables `eDP-1`, exit 0. With 1 monitor → no `eval`,
    exit 0.
  - BATS runs inside the QEMU VM (bats is not on the host), per AGENTS.md.

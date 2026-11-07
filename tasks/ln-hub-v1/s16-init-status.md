---
spec: ../../specs/ln-hub-v1.md
slice_id: S16-init-status
status: planned
---

### Slice 16
- slice_id: S16-init-status
- behavior: While the startup terminal query is pending (~250 ms; keys are not read and are
  dropped, see S14) the status row shows `Initializing…`. When the query finishes it shows
  `Ready`, which clears on the first key press or after ~1 s, then the normal status. The user
  sees why the first keys are ignored. (From the manual VM re-run of S14.)
- seam: three small `App` methods plus the `render` status row, driven by `TestBackend` and
  `tick_at`; the CLI in a pty for the bin wiring.
  - `App::init_started(&mut self)`: the bin calls it before the first frame.
  - `App::init_finished(&mut self, now: std::time::Instant)`: the bin calls it in the loop
    iteration where `ImageView::query_pending()` turns false.
  - `App::tick_at(&mut self, now: std::time::Instant)`: `tick()` is `tick_at(Instant::now())`.
    It does what `tick` does today and also expires `Ready`.
  - Added to the public seam list in `specs/ln-hub-v1.md` when the slice is green.
- test_paths: [tests/s16_init_status.rs, tests/s16_pty.rs, tests/common/**, tests/s14_pty.rs]
- owned_paths: [src/**]
- acceptance_criteria:
  - A fresh `App` shows neither `Initializing…` nor `Ready` (every earlier test keeps its empty
    status row).
  - After `init_started()` the status row (the last row, left-aligned) reads `Initializing…`.
    `tick_at` at any later instant does not clear it: only `init_finished` ends it.
  - After `init_finished(t0)` the status row reads `Ready` and `Initializing…` is gone.
    `tick_at(t0 + 500 ms)` still shows `Ready`; `tick_at(t0 + 1500 ms)` clears it (the row is
    empty, as with no status). `tick()` right after `init_finished(Instant::now())` still shows
    `Ready`.
  - The first key after `init_finished` clears `Ready` without a `tick`, and the key is still
    handled: `?` → the help line, no `Ready`; `j` → the module cursor moves, no `Ready`; the quit
    key → `should_quit()`.
  - After `Ready` has cleared (by key or by time) the row is back to the normal status: a later
    `?` shows the help, an `Enter` apply shows `Applying …`.
  - Priority: the startup text is the lowest-priority text of the left side of the status row,
    below an open prompt and below any status message. It is kept apart from the status message
    and never overwrites it:
    - after `?` (help), `init_started()` and `init_finished(t0)` leave the help line showing and
      neither `Initializing…` nor `Ready` appear, also after `tick_at(t0 + 1500 ms)`;
    - after an `Enter` apply (`Applying wp.png…`), the same calls leave the job status showing;
      once `tick` reports `Done`, `Applied wp.png` shows, never `Ready`;
    - with the `f` prompt open, `init_started()` leaves the prompt line showing.
  - The monitor popup (S15) is an overlay and is not affected: it neither shows nor clears the
    startup text.
  - pty 24x80, silent terminal (as S14): `Initializing…` is in the output together with the
    first frame (the `Theme` title). `Ready` follows within 1 s of the first frame, after
    `Initializing…` in the byte stream. A `q` sent once `Ready` is in the output exits 0 within
    1 s (`Ready` does not swallow the key). No sixel `ESC P` in the output.
- Decisions:
  - `~1 s` is measured with an injectable instant (`init_finished(now)`, `tick_at(now)`) so
    tests move time by arithmetic and never sleep. Detail: both methods take the time, so
    tests move it with `Instant + Duration` and never sleep a second. `tick()`
    keeps its signature and the bin does not change its loop. Rejected: counting `tick` calls
    (the loop period is 50 to 150 ms, so the count is not about 1 s) and a wall-clock sleep in
    the tests (slow, flaky).
  - `Ready` expires on a `tick_at` at or after `now + 1 s` (`READY_FOR`, a constant in
    `src/app.rs`). The tests allow slack (+500 ms still shown, +1500 ms gone); the exact edge is
    unpinned. In the bin the loop ticks every <= ~150 ms, so it clears within ~1.15 s.
  - The startup state is its own `App` field (`Initializing`, `Ready { until }`, nothing), not
    the `status` string. Rendering picks the left text as: open prompt, else status message,
    else startup text. So `Initializing…` / `Ready` can never overwrite a job, help or error
    status, and they need no restore step when they end. A status message set while the startup
    text is up simply hides it.
  - `init_started` and `init_finished` are idempotent; `init_finished` without `init_started`
    shows `Ready` as well. A key delivered to `handle_key` while `Initializing…` is up does not
    end it (the bin never delivers one: it does not read keys during the query). Unpinned.
  - Bin wiring: `app.init_started()` before the first `terminal.draw`; a `was_pending` flag in
    the loop calls `app.init_finished(Instant::now())` once, in the iteration where
    `images.query_pending()` turns false and before that iteration's draw. Because `Ready` is
    drawn before the loop starts reading keys, a `Ready` in the output also means the query
    thread has exited. S18 relies on that. Not covered by a test beyond the pty case.
  - The pty checks look at the raw byte stream of the first draw (ratatui writes a styled run of
    adjacent cells without cursor moves, so `Initializing…` is contiguous). No screen emulator
    is added. The later-frame match on `Ready` works only because every cell of `Ready` differs
    from the `Initializing…` cell it replaces (`R`/`I`, `e`/`n`, `a`/`i`, `d`/`t`, `y`/`i`);
    ratatui's diff skips equal cells and would split the word with a cursor move. A reworded
    text must keep that property (S18's wait for `Ready` depends on it too).
  - The pty `Session` helper moves from `tests/s14_pty.rs` to `tests/common/pty.rs`
    (`pub mod pty` in `tests/common/mod.rs`), a mechanical move done by the test author in the
    RED step; `s14_pty.rs` uses it from there and its tests stay as they are. S16 and S18
    share it. To confirm: copying it into
    `s16_pty.rs` instead is the alternative.
  - Not in scope: a spinner, an elapsed-time counter, replaying the dropped keys (S14), and any
    text for the image decode (`Pending` previews stay as they are).

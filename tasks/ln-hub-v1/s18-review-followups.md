---
spec: ../../specs/ln-hub-v1.md
slice_id: S18-review-followups
status: green
---

### Slice 18
- slice_id: S18-review-followups
- behavior: No user-visible change. Follow-ups from the S14 / S15 code review (all low
  priority): sturdier pty tests, one test rename, one useless test removed, one positive-path pty
  test, and green-preserving refactors of the popup and the query thread.
- seam: the existing ones. Part (a) is tests only (pty, `App` with `TestBackend`). Part (b)
  changes no public signature and no behavior, so the existing tests are its safety net.
- depends_on: S16-init-status (the pty tests wait for `Ready` in the output; S16 also moves the
  pty `Session` helper to `tests/common/pty.rs`). Do S18 after S16.
- test_paths: [tests/s14_pty.rs, tests/s10_toggle.rs, tests/s15_monitor_popup.rs,
  tests/s18_pty_sixel.rs, tests/common/**]
- owned_paths: [src/**]
- acceptance_criteria:
  - (a) tests, through TDD RED (the test author changes them; GOOD RED applies only to the new
    behavior, the rest are edits of green tests that must stay green):
    - `tests/s14_pty.rs`: no fixed sleep races a slow first frame. The first two tests wait
      until `Ready` is in the output (the query window has certainly ended, see S16) and then
      send their keys:
      - `silent_terminal_...`: `q` after `Ready` → exit 0 within 1 s, no sixel `ESC P`. The
        `~300 ms` of S14 becomes "after `Ready`".
      - `keys_after_the_query_deadline_...`: `j`, a short gap (150 ms, it only separates two
        reads and does not race anything), then `q` → exit 0 on the first `q`.
      - `first_key_long_after_...` keeps its time-based wait but counts from `Ready` (`Ready`
        plus 2 s), not from spawn, so a slow start cannot shorten the gap.
      - Test and doc comments no longer say `~300 ms`.
    - `tests/s10_toggle.rs`: `disabling_the_last_enabled_monitor_is_refused_with_a_status` is
      renamed to `disabling_the_last_enabled_monitor_is_refused_with_a_popup` (the refusal moved
      into the popup in S15). Body unchanged.
    - `tests/s15_monitor_popup.rs`: the test
      `preview_image_is_none_while_the_popup_is_open_also_after_a_render` is deleted (see
      Decisions).
    - New `tests/s18_pty_sixel.rs`, pty 24x80 with a terminal that answers the query. The
      harness waits for the query bytes (`ESC [ c`, the DA1 request, is last in the query) in
      the output and then writes the reply `ESC [ ? 62 ; 4 c` (DA1 with sixel) and
      `ESC [ 6 ; 16 ; 8 t` (cell size 8x16) to the pty. HOME has a real
      `Pictures/Wallpapers/a.png` (a small PNG written by the test with the `image` crate).
      - Early exit: the timer starts when `ESC [ c` first shows up in the output, and the reply
        is written right away. `Ready` is in the output within 200 ms of that moment. The
        window's 250 ms deadline counts from the query write, which is before the harness sees
        it, so the two paths differ: early exit is about 10 ms + 30 ms idle + up to 50 ms loop
        sleep (~90 ms); the deadline path is about 250 ms + up to 50 ms (~300 ms). Under 200 ms
        leaves ~100 ms of margin each side, and a broken early exit fails it.
      - No stray keys: after `Ready` the help line is not in the output (the reply bytes did
        not reach the event loop as keys); `l` then `q` → the process exits 0 on the first `q`.
      - Positive path: after `l` (Items focused, `a.png` hovered) a sixel start `ESC P` is in
        the output within 2 s (the mirror of `assert_no_sixel` in S14).
      - The test is a characterization of behavior that already works, so it is green from the
        start (not a GOOD RED). Its comment says so, and why it still earns its place: the
        positive path and the early exit had no pty coverage.
  - (b) refactors, for the implementer, with every test above and all earlier tests green
    before and after:
    - `MonitorsPane::popup(busy: bool)` becomes `popup(&self) -> Option<..>`: `None` when there
      is no usable list or no row under the cursor (so
      `enter_on_monitors_without_a_usable_list_spawns_nothing` keeps passing), else the row plus
      what the cursor row allows, `Confirm(PaneAction)` or `Refuse`. The enum
      `PopupKind { Confirm(PaneAction), Refuse, Busy }` lives with `Popup`; the pane never
      builds `Busy`. `App` makes the popup:
      `Busy` while a job is in flight, else the pane's outcome. The busy rule is then written
      only in `App` (today `busy` is passed into the pane, which also decides the text).
    - `Popup { row: String, kind: PopupKind }` replaces `Popup { lines: Vec<String>, confirm }`.
      Nothing indexes the lines by position; `Popup::lines()` derives the text (`row`, then the
      switch name / the refusal / `Applying…`) and `render_popup` only measures and draws it.
    - Popup key routing moves into `Popup::on_key(self, code: KeyCode, quit_key: char) ->
      PopupNext` with `PopupNext::{Keep(Popup), Close, Confirm(PaneAction)}`. `Esc` and the
      quit key are recognised through `Action::from_key` (`Cancel` and `Quit`), so the
      quit-key mapping is not written a second time in `App::handle_key`; `Enter` on a
      `Confirm` popup confirms; everything else keeps the popup open. `App::handle_key` only
      applies the result.
    - One preview-image-suppression mechanism. `App::preview_image()` keeps its
      `filter(|_| self.popup.is_none())` and the `self.preview_image = None` in `render` goes
      away (or the other way round, see Decisions).
    - `src/image_view.rs`, `query_terminal`: `rustix::io::read` retries on `Errno::INTR` like
      `poll` does (today any error ends the read early); the `poll` timeout is built with
      `Timespec::try_from(left)` instead of the hand-written `tv_sec: 0, tv_nsec`. If
      `TryFrom<Duration>` is not available for `Timespec` in the pinned rustix (1.1.6), keep
      the literal and drop that item.
    - `src/image_view.rs` module doc gets two notes: (1) a reply that arrives after the
      ~250 ms window is read by the event loop as key bytes (S14 drops only what arrives inside
      the window); (2) `Picker::from_fontsize` is deprecated and kept behind
      `#[allow(deprecated)]` because it is the only public way to set the parsed cell size:
      revisit on the next `ratatui-image` major bump. The `allow` line points to the note.
    - `pub fn term_query::parse` stays (see Decisions); its doc says it is the one-shot
      convenience used by tests and callers that have one buffer.
- Decisions:
  - `preview_image_is_none_while_the_popup_is_open...` is dropped, not strengthened. Monitors
    never carry a preview image, so the assertion cannot fail, and the only popup lives in the
    Monitors pane. A test that could fail needs a popup over a pane with an image (a Theme
    popup, a non-goal of S15). When such a popup is added its slice adds the test. The guard
    in `App::preview_image()` stays as untested insurance, which the S15 notes already say.
    To confirm: keeping the test as a documented regression guard is the alternative.
  - Suppression mechanism kept: the filter in `App::preview_image()`. It is derived from the
    current popup state, so it is right between a key and the next `render`, while the
    `render`-time reset only holds after a render. The bin renders before it reads the image,
    so behavior is the same either way.
  - `Ready` in the output is the wait signal because S16 draws it only after the query thread
    has exited and before the loop reads keys. A key sent after it cannot land in the query
    window.
  - `term_query::parse` stays public: `tests/s14_term_query.rs` calls it about twenty times as
    the one-shot seam, `Parser` is the streaming seam, and removing it would only rewrite
    green tests. Not used from `src/`.
  - The `Popup` refactors may move `Popup` into its own `src/popup.rs`; the module layout is the
    implementer's call as long as `lib.rs` exports stay the same.
  - Not in scope: replaying keys that arrive in the query window, a screen emulator for the pty
    tests, a Theme popup, and a `portable-pty` upgrade.
- Follow-ups (deferred, non-blocking review items):
  - Popup fields `row` / `kind` stay `pub(crate)` because `MonitorsPane::popup` builds
    `Popup { row, kind }`; a `Popup::new` would allow private fields.
  - `PopupKind::Confirm(PaneAction)` admits an unreachable `Status` arm (`popup.rs`); a narrower
    payload would remove it.
  - The doc comment on `struct Popup` still says "An open confirmation popup"; it also covers
    refusal and busy.
  - The pty test helpers (`wait_for` / `find`, kill-then-panic) are duplicated across
    `tests/s16_pty.rs`, `tests/s18_pty_sixel.rs` and `tests/common/pty.rs`.
  - `tests/s16_init_status.rs` and `tests/s17_help_status_overlap.rs` are not rustfmt-clean.

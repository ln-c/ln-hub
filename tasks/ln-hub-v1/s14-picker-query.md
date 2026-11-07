---
spec: ../../specs/ln-hub-v1.md
slice_id: S14-picker-query
status: green
---

### Slice 14
- slice_id: S14-picker-query
- behavior: The terminal capability query has a hard deadline and never freezes the UI or eats
  keys. ln-hub writes the query itself on a background thread, reads replies with `poll(2)`,
  and builds the `Picker` from the parsed result. (From manual VM run item 9.)
- seam: `ln_hub::term_query` (new lib module: reply parser, bytes in, capabilities out) and the
  CLI in a pty. `ImageView` stays bin-only and only calls the query.
- test_paths: [tests/s14_pty.rs, tests/s14_term_query.rs]
- owned_paths: [src/**, Cargo.toml]
- root cause: `Picker::from_query_stdio` (src/image_view.rs, `ImageView::new`) spawns a thread
  doing a blocking `io::stdin().read()`. A terminal that never answers freezes the UI for about
  2 s after the first frame. Keys typed in that window are eaten, and each restarts the timeout.
  After the timeout the orphaned reader thread eats the first key.
- acceptance_criteria:
  - pty 24x80 that never answers queries: the first frame is drawn. `q` sent ~300 ms after
    spawn → the process exits 0 within 1 s of the key. The output has no sixel `ESC P`.
  - Same pty: keys sent after the query deadline are not lost. `j` then `q` → `q` exits on the
    first press (no second `q` needed).
  - Parser (`tests/s14_term_query.rs`, no tty):
    - a DA1 reply (`ESC [ ? 62 ; 4 ; 9 c`) that lists attribute 4 → sixel;
    - a kitty graphics OK reply (`ESC _ G i=… ; OK ESC \`) → kitty;
    - a `ESC [ 6 ; <h> ; <w> t` reply → font size `(w, h)`;
    - replies split across several reads, and replies mixed with key bytes, parse the same as
      one clean read (feed the bytes in chunks);
    - empty input, or a DA1 reply without 4 and without a kitty reply → halfblocks, no font
      size (the picker keeps its default font size).
  - The query thread reads only until the deadline (~250 ms, `poll(2)` on the tty fd) and
    exits, leaving no reader on stdin: after it reports done, the next stdin byte goes to the
    event loop.
  - Same silent pty: the loop keeps drawing during the query window, so the process stays
    responsive (the first-frame and exit criteria above cover it; no output stall is expected).
- Decisions:
  - Own query instead of `Picker::from_query_stdio`. The picker is built as
    `Picker::halfblocks()` plus `set_protocol_type` (sixel / kitty) and the parsed font size.
    No reply → halfblocks. `poll(2)` comes from a direct `rustix` dependency (`libc` is also
    allowed).
  - The query runs on a background thread, started after the first frame, and reports the parsed
    capabilities over a channel. While it runs (<= ~250 ms) the thread owns every tty read. The
    main loop keeps ticking and redrawing, so the UI does not freeze, but it does not read
    crossterm events until the thread reports done: there is never a concurrent stdin reader.
    The thread uses `poll(2)` with the deadline, sends the result and exits; no reader is left
    behind. The main loop builds the `Picker` when the result arrives and uses halfblocks until
    then. The "never concurrent" and "loop keeps redrawing" parts are covered by the pty
    tests (no frozen window, no eaten key); the thread's exit before event reads resume is a
    design rule, not separately testable.
  - pty crate: `portable-pty` as a dev-dependency (the spawned bin gets a real controlling tty,
    and the test controls the window size and timing). Confirmed.
  - Key bytes that arrive within the deadline and are not replies are dropped and documented,
    not replayed. The window is ~250 ms right after the first frame; replaying would need a
    parser for key escapes. Confirmed.
  - The query asks for DA1, kitty graphics and the cell size (`CSI 16 t`) in one write.
    A DA1 reply is the sentinel every terminal answers, so the read may end early once it has
    arrived and the input has gone idle. The deadline is the upper bound.
  - The timing tests allow slack: `q` is sent at ~300 ms, i.e. after the 250 ms deadline plus
    scheduling margin.

- Manual VM evidence (2026-11-07): silent terminal falls back to halfblocks with no freeze and `q`
  quits on the first press, foot keeps sixel; keys inside the 250 ms window of a silent terminal
  are dropped by design. Screenshots in `tests/manual/s14/`, notes in the "Manual VM re-run" block
  of `s13-bin.md`.

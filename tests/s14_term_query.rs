//! S14-picker-query: the terminal reply parser, bytes in and capabilities out, no tty needed.
//!
//! API under test (`ln_hub::term_query`):
//! - `Parser::new()` / `Parser::default()`: incremental parser; `feed(&mut self, &[u8])` takes
//!   one read's bytes (any split, any junk), `capabilities(&self) -> Capabilities` is the
//!   result so far.
//! - `parse(&[u8]) -> Capabilities`: one-shot convenience (a fresh parser fed once).
//! - `Capabilities { protocol: Protocol, font_size: Option<(u16, u16)> }` (width, height),
//!   `Protocol::{Sixel, Kitty, Halfblocks}`; all `Debug + Clone + PartialEq`.

use ln_hub::term_query::{Capabilities, Parser, Protocol, parse};

const DA1_SIXEL: &[u8] = b"\x1b[?62;4;9c";
const DA1_PLAIN: &[u8] = b"\x1b[?62;9c";
const KITTY_OK: &[u8] = b"\x1b_Gi=31;OK\x1b\\";
/// `CSI 6 ; <height> ; <width> t`: cell height 20, width 10.
const CELL_20_10: &[u8] = b"\x1b[6;20;10t";

fn halfblocks_default() -> Capabilities {
    Capabilities {
        protocol: Protocol::Halfblocks,
        font_size: None,
    }
}

fn feed_chunks(chunks: &[&[u8]]) -> Capabilities {
    let mut p = Parser::new();
    for c in chunks {
        p.feed(c);
    }
    p.capabilities()
}

#[test]
fn da1_listing_attribute_4_means_sixel() {
    // Catches: ignoring DA1 entirely, or never mapping attribute 4 to sixel.
    let caps = parse(DA1_SIXEL);
    assert_eq!(caps.protocol, Protocol::Sixel);
    assert_eq!(caps.font_size, None);
}

#[test]
fn da1_attribute_4_is_found_first_and_last_in_the_list() {
    // Catches: only looking at one list position (e.g. skipping the terminal-class parameter
    // or the last parameter before `c`).
    assert_eq!(parse(b"\x1b[?4;6c").protocol, Protocol::Sixel);
    assert_eq!(parse(b"\x1b[?62;9;4c").protocol, Protocol::Sixel);
}

#[test]
fn da1_without_4_is_halfblocks_even_with_14_24_41_attributes() {
    // Catches: substring matching ("4" inside "14", "24", "41") reporting sixel.
    let caps = parse(b"\x1b[?62;14;24;41c");
    assert_eq!(caps, halfblocks_default());
}

#[test]
fn da1_without_4_and_no_kitty_reply_is_halfblocks_without_font_size() {
    // Catches: defaulting to sixel when DA1 arrives, or inventing a font size.
    assert_eq!(parse(DA1_PLAIN), halfblocks_default());
}

#[test]
fn kitty_graphics_ok_reply_means_kitty() {
    // Catches: kitty replies being ignored (fallback to halfblocks on kitty/ghostty).
    let caps = parse(KITTY_OK);
    assert_eq!(caps.protocol, Protocol::Kitty);
    assert_eq!(caps.font_size, None);
}

#[test]
fn kitty_ok_reply_followed_by_da1_is_kitty() {
    // Catches: the DA1 sentinel (which always arrives last) overriding an earlier kitty OK.
    assert_eq!(
        parse(&[KITTY_OK, DA1_PLAIN].concat()).protocol,
        Protocol::Kitty
    );
}

#[test]
fn kitty_ok_reply_wins_over_da1_listing_sixel() {
    // Catches: preferring sixel when a terminal answers both (kitty is the better protocol).
    assert_eq!(
        parse(&[KITTY_OK, DA1_SIXEL].concat()).protocol,
        Protocol::Kitty
    );
}

#[test]
fn kitty_error_reply_is_not_kitty() {
    // Catches: treating any `ESC _ G` reply as support instead of requiring `OK`.
    let caps = parse(&[&b"\x1b_Gi=31;ENOTSUPP:no\x1b\\"[..], DA1_PLAIN].concat());
    assert_eq!(caps, halfblocks_default());
}

#[test]
fn cell_size_reply_gives_font_size_width_then_height() {
    // Catches: swapped (h, w) order, or the font size not parsed at all.
    let caps = parse(&[CELL_20_10, DA1_PLAIN].concat());
    assert_eq!(caps.font_size, Some((10, 20)));
    assert_eq!(caps.protocol, Protocol::Halfblocks);
}

#[test]
fn all_three_replies_together() {
    // Catches: one reply kind swallowing the next one in the stream.
    let caps = parse(&[KITTY_OK, CELL_20_10, DA1_SIXEL].concat());
    assert_eq!(
        caps,
        Capabilities {
            protocol: Protocol::Kitty,
            font_size: Some((10, 20)),
        }
    );
}

#[test]
fn empty_input_is_halfblocks_without_font_size() {
    // Catches: a silent terminal producing a sixel/kitty picker or a bogus font size.
    assert_eq!(parse(b""), halfblocks_default());
    assert_eq!(Parser::new().capabilities(), halfblocks_default());
}

#[test]
fn key_bytes_alone_are_ignored() {
    // Catches: typed keys (incl. an arrow's CSI) being mistaken for a reply.
    assert_eq!(parse(b"jjk\x1b[Aq"), halfblocks_default());
}

fn full_reply() -> Vec<u8> {
    [CELL_20_10, KITTY_OK, DA1_SIXEL].concat()
}

fn expected_full() -> Capabilities {
    Capabilities {
        protocol: Protocol::Kitty,
        font_size: Some((10, 20)),
    }
}

#[test]
fn reply_fed_one_byte_at_a_time_parses_like_one_read() {
    // Catches: a stateless parser that loses a reply cut across reads.
    let bytes = full_reply();
    let chunks: Vec<&[u8]> = bytes.chunks(1).collect();
    assert_eq!(feed_chunks(&chunks), expected_full());
}

#[test]
fn reply_split_at_every_boundary_parses_like_one_read() {
    // Catches: a parse that breaks when the cut falls inside an escape sequence.
    let bytes = full_reply();
    for cut in 0..=bytes.len() {
        let (a, b) = bytes.split_at(cut);
        assert_eq!(feed_chunks(&[a, b]), expected_full(), "cut at {cut}");
    }
}

#[test]
fn sixel_font_reply_split_at_every_boundary_and_in_threes() {
    // Catches: chunk handling that only works for the kitty path, or for 2-way splits.
    let bytes = [CELL_20_10, DA1_SIXEL].concat();
    let want = Capabilities {
        protocol: Protocol::Sixel,
        font_size: Some((10, 20)),
    };
    for cut in 0..=bytes.len() {
        let (a, b) = bytes.split_at(cut);
        assert_eq!(feed_chunks(&[a, b]), want, "cut at {cut}");
    }
    let chunks: Vec<&[u8]> = bytes.chunks(3).collect();
    assert_eq!(feed_chunks(&chunks), want);
}

#[test]
fn replies_mixed_with_key_bytes_parse_like_a_clean_read() {
    // Catches: key bytes before, between and after replies corrupting the parse
    // (including an arrow key's `ESC [ A`, which starts like a reply).
    let mixed = [
        &b"j"[..],
        CELL_20_10,
        b"k\x1b[A",
        KITTY_OK,
        b"q",
        DA1_SIXEL,
        b"x",
    ]
    .concat();
    assert_eq!(parse(&mixed), expected_full());
    let chunks: Vec<&[u8]> = mixed.chunks(2).collect();
    assert_eq!(feed_chunks(&chunks), expected_full());
}

#[test]
fn partial_reply_without_terminator_is_not_a_capability() {
    // Catches: acting on an unfinished DA1 (`ESC [ ? 62 ; 4` with no `c`) as sixel.
    assert_eq!(parse(b"\x1b[?62;4"), halfblocks_default());
}

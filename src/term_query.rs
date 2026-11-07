//! Parser for the terminal's replies to ln-hub's capability query (DA1, kitty graphics query,
//! `CSI 16 t`). Bytes in, capabilities out; no tty access.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Sixel,
    Kitty,
    Halfblocks,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capabilities {
    pub protocol: Protocol,
    /// Cell size in pixels, (width, height).
    pub font_size: Option<(u16, u16)>,
}

/// Accumulates reply bytes (any split, any junk) and scans them on demand.
#[derive(Default)]
pub struct Parser {
    buf: Vec<u8>,
}

#[derive(Default)]
struct Scan {
    sixel: bool,
    kitty: bool,
    da1: bool,
    font_size: Option<(u16, u16)>,
}

impl Parser {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    /// Whether the DA1 reply (always answered last) has arrived.
    pub fn da1_seen(&self) -> bool {
        self.scan().da1
    }

    pub fn capabilities(&self) -> Capabilities {
        let scan = self.scan();
        let protocol = if scan.kitty {
            Protocol::Kitty
        } else if scan.sixel {
            Protocol::Sixel
        } else {
            Protocol::Halfblocks
        };
        Capabilities {
            protocol,
            font_size: scan.font_size,
        }
    }

    fn scan(&self) -> Scan {
        let mut scan = Scan::default();
        let b = &self.buf;
        let mut i = 0;
        while i < b.len() {
            if b[i] != 0x1b {
                i += 1;
                continue;
            }
            match b.get(i + 1) {
                Some(b'[') => {
                    let start = i + 2;
                    let Some(len) = b[start.min(b.len())..]
                        .iter()
                        .position(|c| (0x40..=0x7e).contains(c))
                    else {
                        break;
                    };
                    let params = &b[start..start + len];
                    match b[start + len] {
                        b'c' if params.first() == Some(&b'?') => {
                            scan.da1 = true;
                            scan.sixel |= params[1..].split(|&c| c == b';').any(|p| p == b"4");
                        }
                        b't' => {
                            if let Some(size) = cell_size(params) {
                                scan.font_size = Some(size);
                            }
                        }
                        _ => {}
                    }
                    i = start + len + 1;
                }
                Some(b'_') => {
                    let start = i + 2;
                    let Some(len) = b[start..].windows(2).position(|w| w == b"\x1b\\") else {
                        break;
                    };
                    let body = &b[start..start + len];
                    scan.kitty |= body.starts_with(b"G") && body.ends_with(b";OK");
                    i = start + len + 2;
                }
                Some(_) => i += 1,
                None => break,
            }
        }
        scan
    }
}

/// `6;<height>;<width>` -> (width, height).
fn cell_size(params: &[u8]) -> Option<(u16, u16)> {
    let text = std::str::from_utf8(params).ok()?;
    let mut parts = text.split(';');
    if parts.next()? != "6" {
        return None;
    }
    let h = parts.next()?.parse().ok()?;
    let w = parts.next()?.parse().ok()?;
    parts.next().is_none().then_some((w, h))
}

/// One-shot convenience over `Parser` for tests and callers that have a single buffer.
pub fn parse(bytes: &[u8]) -> Capabilities {
    let mut parser = Parser::new();
    parser.feed(bytes);
    parser.capabilities()
}

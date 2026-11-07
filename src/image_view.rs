//! Binary-only glue that draws the hovered image through ratatui-image.
//!
//! Decoding happens on a worker thread; the resize/encode step inside `StatefulImage` stays
//! synchronous (it only reruns when the image or its area changes, and the worker downscales
//! huge sources first so that step is cheap).

use std::collections::VecDeque;
use std::io::{self, Write};
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use image::DynamicImage;
use ln_hub::PreviewImage;
use ln_hub::term_query::{self, Capabilities, Protocol};
use ratatui::Frame;
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::{FilterType, Resize, StatefulImage};

/// Number of most recently requested images kept (decoded, encoded, or failed).
const CACHE_CAP: usize = 8;
/// Decoded images are shrunk to fit this many pixels per side before the UI thread sees them.
const MAX_SIDE: u32 = 1600;

enum Entry {
    Pending,
    Failed,
    Ready(Box<StatefulProtocol>),
}

/// Worker reply for one requested path.
enum Decoded {
    Image(PathBuf, DynamicImage),
    Failed(PathBuf),
    /// A newer request superseded this one; the cache entry is dropped so it can be re-requested.
    Skipped(PathBuf),
}

pub struct ImageView {
    picker: Picker,
    /// The capability query's result; `Some` while the query thread is still running.
    query: Option<Receiver<Capabilities>>,
    requests: Sender<PathBuf>,
    decoded: Receiver<Decoded>,
    /// Oldest first.
    cache: VecDeque<(PathBuf, Entry)>,
}

fn decode(path: &Path) -> Option<DynamicImage> {
    let img = image::ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?;
    Some(if img.width() > MAX_SIDE || img.height() > MAX_SIDE {
        img.thumbnail(MAX_SIDE, MAX_SIDE)
    } else {
        img
    })
}

impl ImageView {
    /// Starts the terminal capability query and the decode worker; halfblocks are used until
    /// the query result arrives. Call after the first frame is drawn.
    pub fn new() -> Self {
        let (query_tx, query) = mpsc::channel();
        thread::spawn(move || {
            let _ = query_tx.send(query_terminal());
        });
        let (requests, request_rx) = mpsc::channel::<PathBuf>();
        let (decoded_tx, decoded) = mpsc::channel();
        thread::spawn(move || {
            // Only the newest queued request is decoded; older ones are reported as skipped.
            while let Ok(mut path) = request_rx.recv() {
                let mut replies = Vec::new();
                while let Ok(newer) = request_rx.try_recv() {
                    replies.push(Decoded::Skipped(std::mem::replace(&mut path, newer)));
                }
                // Superseded requests are answered before the (slow) decode of the newest.
                for reply in replies {
                    if decoded_tx.send(reply).is_err() {
                        return;
                    }
                }
                let decoded = panic::catch_unwind(AssertUnwindSafe(|| decode(&path))).ok();
                let reply = match decoded.flatten() {
                    Some(img) => Decoded::Image(path, img),
                    None => Decoded::Failed(path),
                };
                if decoded_tx.send(reply).is_err() {
                    return;
                }
            }
        });
        Self {
            picker: Picker::halfblocks(),
            query: Some(query),
            requests,
            decoded,
            cache: VecDeque::new(),
        }
    }

    /// Whether the query thread may still be reading stdin (nobody else may read it then).
    pub fn query_pending(&self) -> bool {
        self.query.is_some()
    }

    /// Applies the query result and moves finished decodes into the cache. Call once per loop
    /// iteration.
    pub fn poll(&mut self) {
        if let Some(query) = &self.query {
            match query.try_recv() {
                Ok(caps) => {
                    self.picker = picker_from(&caps);
                    // Anything built with the halfblocks placeholder picker is re-requested.
                    self.cache.clear();
                    self.query = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => self.query = None,
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        while let Ok(reply) = self.decoded.try_recv() {
            // The entry may have been evicted while decoding; then the result is dropped.
            let (Decoded::Image(path, _) | Decoded::Failed(path) | Decoded::Skipped(path)) = &reply;
            let Some(i) = self.cache.iter().position(|(p, _)| p == path) else {
                continue;
            };
            match reply {
                Decoded::Image(_, img) => {
                    let proto = self.picker.new_resize_protocol(img);
                    self.cache[i].1 = Entry::Ready(Box::new(proto));
                }
                Decoded::Failed(_) => self.cache[i].1 = Entry::Failed,
                Decoded::Skipped(_) => {
                    self.cache.remove(i);
                }
            }
        }
    }

    /// Draws `preview` if its image is ready, requesting the decode the first time it is seen.
    /// Failed paths stay cached (as `Failed`) so they are not requested again.
    pub fn render(&mut self, f: &mut Frame, preview: &PreviewImage) {
        if let Some(i) = self.cache.iter().position(|(p, _)| *p == preview.path) {
            // Most recently used goes to the back.
            if let Some(hit) = self.cache.remove(i) {
                self.cache.push_back(hit);
            }
            if let Some((_, Entry::Ready(proto))) = self.cache.back_mut() {
                let widget =
                    StatefulImage::default().resize(Resize::Scale(Some(FilterType::Triangle)));
                f.render_stateful_widget(widget, preview.area, proto.as_mut());
            }
        } else {
            if self.cache.len() >= CACHE_CAP {
                self.cache.pop_front();
            }
            let sent = self.requests.send(preview.path.clone()).is_ok();
            let entry = if sent { Entry::Pending } else { Entry::Failed };
            self.cache.push_back((preview.path.clone(), entry));
        }
    }
}

/// Writes the capability query to the terminal and collects its replies from stdin for at most
/// `WINDOW`. Runs on a thread that has exited (leaving no stdin reader) by the time the result
/// is sent. Key bytes typed inside the window are not replies and are dropped.
fn query_terminal() -> Capabilities {
    use rustix::event::Timespec;
    use rustix::event::{PollFd, PollFlags, poll};
    use std::time::Instant;

    const WINDOW: Duration = Duration::from_millis(250);
    const IDLE: Duration = Duration::from_millis(30);
    // Kitty graphics query, cell size in pixels, then DA1 (answered last by every terminal).
    const QUERY: &[u8] = b"\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\\x1b[16t\x1b[c";

    let mut out = io::stdout();
    let mut parser = term_query::Parser::new();
    if out.write_all(QUERY).and_then(|()| out.flush()).is_err() {
        return parser.capabilities();
    }
    let deadline = Instant::now() + WINDOW;
    let mut buf = [0u8; 256];
    loop {
        let mut left = deadline.saturating_duration_since(Instant::now());
        if parser.da1_seen() {
            left = left.min(IDLE);
        }
        if left.is_zero() {
            break;
        }
        let stdin = rustix::stdio::stdin();
        let mut fds = [PollFd::new(&stdin, PollFlags::IN)];
        let timeout = Timespec {
            tv_sec: 0,
            tv_nsec: i64::from(left.subsec_nanos()),
        };
        match poll(&mut fds, Some(&timeout)) {
            Ok(0) => {
                if parser.da1_seen() || Instant::now() >= deadline {
                    break;
                }
            }
            Ok(_) => match rustix::io::read(stdin, &mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => parser.feed(&buf[..n]),
            },
            Err(rustix::io::Errno::INTR) => {}
            Err(_) => break,
        }
    }
    parser.capabilities()
}

fn picker_from(caps: &Capabilities) -> Picker {
    #[allow(deprecated)] // the only public way to set the parsed cell size
    let mut picker = match caps.font_size {
        Some(size) => Picker::from_fontsize(size.into()),
        None => Picker::halfblocks(),
    };
    picker.set_protocol_type(match caps.protocol {
        Protocol::Sixel => ProtocolType::Sixel,
        Protocol::Kitty => ProtocolType::Kitty,
        Protocol::Halfblocks => ProtocolType::Halfblocks,
    });
    picker
}

//! Binary-only glue that draws the hovered image through ratatui-image.
//!
//! Decoding happens on a worker thread, and so does the resize+encode step (S22): each cached
//! image is a `ThreadProtocol`, whose `StatefulImage` render sends a `ResizeRequest` instead of
//! encoding on the UI thread. `render` forwards that request, tagged with the entry's path and
//! serial, to one encode worker; `poll` routes the response back by that tag and drops it when the
//! entry is gone or was re-created. The preview is blank while its encode is pending.
//!
//! When `LN_HUB_TIMING=<file>` is set the worker appends `encode <cols>x<rows> <ms>ms` per
//! finished encode (see `TimingLog`; `main.rs` adds the `draw` lines).
//!
//! Notes:
//! - A terminal reply that arrives after the ~250 ms query window is read by the event loop as
//!   key bytes; only what arrives inside the window is dropped.
//! - `Picker::from_fontsize` is deprecated and kept behind `#[allow(deprecated)]` because it is
//!   the only public way to set the parsed cell size; revisit on the next `ratatui-image` major
//!   bump.

use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use image::DynamicImage;
use ln_hub::PreviewImage;
use ln_hub::term_query::{self, Capabilities, Protocol};
use ratatui::Frame;
use ratatui::layout::Size;
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::thread::{ResizeRequest, ResizeResponse, ThreadProtocol};
use ratatui_image::{FilterType, Resize, ResizeEncodeRender, StatefulImage};

/// Number of most recently requested images kept (decoded, encoded, or failed).
const CACHE_CAP: usize = 8;
/// Decoded images are shrunk to fit this many pixels per side before the UI thread sees them.
const MAX_SIDE: u32 = 1600;

enum Entry {
    Pending,
    Failed,
    Ready(Box<Ready>),
}

/// A decoded image. `proto` sends its resize requests to `requests`, which `render` drains into
/// the encode worker.
struct Ready {
    proto: ThreadProtocol,
    /// Unique per `ImageView`, so a response for an evicted (or evicted and re-created) entry
    /// never matches; `ThreadProtocol` ids restart at 0 per protocol and cannot tell entries apart.
    serial: u64,
    requests: Receiver<ResizeRequest>,
}

/// One resize+encode for the worker, tagged with the entry it belongs to and the size asked for
/// (a `ResizeRequest` exposes neither).
struct EncodeJob {
    path: PathBuf,
    serial: u64,
    size: Size,
    request: ResizeRequest,
}

/// The worker's reply; `response` is `None` when the encode failed or panicked.
struct Encoded {
    path: PathBuf,
    serial: u64,
    response: Option<ResizeResponse>,
}

/// The opt-in `LN_HUB_TIMING` log: an append-mode file shared by the UI and encode threads.
#[derive(Clone)]
pub struct TimingLog {
    file: Arc<Mutex<File>>,
}

impl TimingLog {
    /// The log at `$LN_HUB_TIMING`; `None` (timing off) when the variable is unset or the file
    /// cannot be opened. No directory is created.
    pub fn from_env() -> Option<TimingLog> {
        let path = std::env::var_os("LN_HUB_TIMING")?;
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .ok()?;
        Some(TimingLog {
            file: Arc::new(Mutex::new(file)),
        })
    }

    /// Appends `text` and a newline in one write; errors are ignored.
    pub fn line(&self, text: &str) {
        let line = format!("{text}\n");
        if let Ok(mut file) = self.file.lock() {
            let _ = file.write_all(line.as_bytes());
        }
    }
}

/// `d` in milliseconds with exactly one decimal, e.g. `12.3ms`.
pub fn fmt_ms(d: Duration) -> String {
    format!("{:.1}ms", d.as_secs_f64() * 1000.0)
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
    encode_jobs: Sender<EncodeJob>,
    encoded: Receiver<Encoded>,
    /// Encode jobs sent to the worker whose reply has not been drained yet.
    in_flight: usize,
    next_serial: u64,
}

/// The encode worker: runs each job's `resize_encode`, logs its time when timing is on (a failed
/// or panicking encode too), and replies. Exits when either channel closes.
fn encode_worker(jobs: Receiver<EncodeJob>, replies: Sender<Encoded>, timing: Option<TimingLog>) {
    while let Ok(EncodeJob {
        path,
        serial,
        size,
        request,
    }) = jobs.recv()
    {
        let start = Instant::now();
        // `ResizeRequest::resize_encode` panics on an empty size (nothing gets encoded), so that
        // is answered as a failure without calling it.
        let response = if size.width == 0 || size.height == 0 {
            None
        } else {
            panic::catch_unwind(AssertUnwindSafe(|| request.resize_encode()))
                .ok()
                .and_then(Result::ok)
        };
        if let Some(timing) = &timing {
            timing.line(&format!(
                "encode {}x{} {}",
                size.width,
                size.height,
                fmt_ms(start.elapsed())
            ));
        }
        let reply = Encoded {
            path,
            serial,
            response,
        };
        if replies.send(reply).is_err() {
            return;
        }
    }
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
    /// Starts the terminal capability query, the decode worker and the encode worker; halfblocks
    /// are used until the query result arrives. Call after the first frame is drawn. With
    /// `timing`, every finished encode is logged.
    pub fn new(timing: Option<TimingLog>) -> Self {
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
        let (encode_jobs, job_rx) = mpsc::channel();
        let (encoded_tx, encoded) = mpsc::channel();
        thread::spawn(move || encode_worker(job_rx, encoded_tx, timing));
        Self {
            picker: Picker::halfblocks(),
            query: Some(query),
            requests,
            decoded,
            cache: VecDeque::new(),
            encode_jobs,
            encoded,
            in_flight: 0,
            next_serial: 0,
        }
    }

    /// Whether an encode sent to the worker has not been answered yet (the loop then polls for
    /// keys more often, so the finished encode is drawn soon).
    pub fn encode_pending(&self) -> bool {
        self.in_flight > 0
    }

    /// Whether the query thread may still be reading stdin (nobody else may read it then).
    pub fn query_pending(&self) -> bool {
        self.query.is_some()
    }

    /// Applies the query result, moves finished decodes into the cache and applies finished
    /// encodes to their entries. Call once per loop iteration.
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
                    let (entry_tx, requests) = mpsc::channel();
                    let proto =
                        ThreadProtocol::new(entry_tx, Some(self.picker.new_resize_protocol(img)));
                    let serial = self.next_serial;
                    self.next_serial += 1;
                    self.cache[i].1 = Entry::Ready(Box::new(Ready {
                        proto,
                        serial,
                        requests,
                    }));
                }
                Decoded::Failed(_) => self.cache[i].1 = Entry::Failed,
                Decoded::Skipped(_) => {
                    self.cache.remove(i);
                }
            }
        }
        loop {
            let Encoded {
                path,
                serial,
                response,
            } = match self.encoded.try_recv() {
                Ok(reply) => reply,
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.in_flight = 0;
                    break;
                }
            };
            self.in_flight = self.in_flight.saturating_sub(1);
            // Routed by the tag only: an evicted, re-created or cleared entry drops the reply.
            let Some((_, entry)) = self
                .cache
                .iter_mut()
                .find(|(p, e)| *p == path && matches!(e, Entry::Ready(r) if r.serial == serial))
            else {
                continue;
            };
            let applied = match (&mut *entry, response) {
                (Entry::Ready(ready), Some(response)) => {
                    ready.proto.update_resized_protocol(response)
                }
                _ => false,
            };
            // A failed encode consumed the protocol (and a rejected response, an id mismatch that
            // cannot happen with one request in flight, would leave it empty); the entry becomes
            // `Failed`, blank and not retried, instead of pending forever.
            if !applied {
                *entry = Entry::Failed;
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
            let mut send_failed = false;
            if let Some((path, Entry::Ready(ready))) = self.cache.back_mut() {
                let resize = Resize::Scale(Some(FilterType::Triangle));
                // The size this render asks the worker for (a `ResizeRequest` does not say).
                let size = ready
                    .proto
                    .needs_resize(&resize, preview.area.into())
                    .unwrap_or(preview.area.into());
                let widget = StatefulImage::default().resize(resize);
                f.render_stateful_widget(widget, preview.area, &mut ready.proto);
                // Forwarded within the frame, not once per loop iteration.
                while let Ok(request) = ready.requests.try_recv() {
                    let job = EncodeJob {
                        path: path.clone(),
                        serial: ready.serial,
                        size,
                        request,
                    };
                    if self.encode_jobs.send(job).is_ok() {
                        self.in_flight += 1;
                    } else {
                        send_failed = true;
                    }
                }
            }
            // The protocol went with the unsent request; never pending forever.
            if send_failed && let Some((_, entry)) = self.cache.back_mut() {
                *entry = Entry::Failed;
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
        // `left` is at most `WINDOW`, so the conversion cannot fail; ending the query is the
        // safe fallback.
        let Ok(timeout) = Timespec::try_from(left) else {
            break;
        };
        match poll(&mut fds, Some(&timeout)) {
            Ok(0) => {
                if parser.da1_seen() || Instant::now() >= deadline {
                    break;
                }
            }
            Ok(_) => match rustix::io::read(stdin, &mut buf) {
                Ok(0) => break,
                Ok(n) => parser.feed(&buf[..n]),
                Err(rustix::io::Errno::INTR) => {}
                Err(_) => break,
            },
            Err(rustix::io::Errno::INTR) => {}
            Err(_) => break,
        }
    }
    parser.capabilities()
}

fn picker_from(caps: &Capabilities) -> Picker {
    #[allow(deprecated)] // see the module doc note on `from_fontsize`
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

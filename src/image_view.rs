//! Binary-only glue that draws the hovered image through ratatui-image.
//!
//! Decoding happens on a worker thread; the resize/encode step inside `StatefulImage` stays
//! synchronous (it only reruns when the image or its area changes, and the worker downscales
//! huge sources first so that step is cheap).

use std::collections::VecDeque;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use image::DynamicImage;
use ln_hub::PreviewImage;
use ratatui::Frame;
use ratatui_image::picker::Picker;
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
    /// Queries the terminal for graphics support (falls back to halfblocks) and starts the
    /// decode worker. Call after the first frame is drawn.
    pub fn new() -> Self {
        let picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());
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
            picker,
            requests,
            decoded,
            cache: VecDeque::new(),
        }
    }

    /// Moves finished decodes into the cache. Call once per loop iteration.
    pub fn poll(&mut self) {
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

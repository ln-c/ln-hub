//! The Monitors items pane: monitors listed from `hyprctl monitors all -j`.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, List, ListItem, Paragraph};
use serde::Deserialize;

use crate::system::System;

/// Shown in the items column when `hyprctl` fails or prints something unreadable.
const UNAVAILABLE: &str = "hyprctl unavailable";

/// One entry of `hyprctl monitors all -j`; other fields are ignored.
#[derive(Deserialize)]
struct Monitor {
    name: String,
    width: u32,
    height: u32,
    #[serde(rename = "refreshRate")]
    refresh_rate: f64,
    disabled: bool,
}

impl Monitor {
    fn row(&self) -> String {
        format!(
            "{}  {}x{}@{:.0}  {}",
            self.name,
            self.width,
            self.height,
            self.refresh_rate,
            if self.disabled { "off" } else { "on" }
        )
    }
}

/// Runs `hyprctl` and parses its monitor list; `None` on a run error or unreadable output.
fn query(sys: &dyn System) -> Option<Vec<Monitor>> {
    let out = sys.run("hyprctl", &["monitors", "all", "-j"]).ok()?;
    serde_json::from_slice(&out.stdout).ok()
}

pub(crate) struct MonitorsPane {
    /// `None` when the last query failed.
    monitors: Option<Vec<Monitor>>,
}

impl MonitorsPane {
    /// An empty pane; nothing is queried until `load`.
    pub(crate) fn new() -> MonitorsPane {
        MonitorsPane {
            monitors: Some(Vec::new()),
        }
    }

    /// Queries `hyprctl` and replaces the list.
    pub(crate) fn load(&mut self, sys: &dyn System) {
        self.monitors = query(sys);
    }

    /// Draws the pane's two columns; the preview is empty.
    pub(crate) fn render(&self, f: &mut Frame, items_area: Rect, preview_area: Rect) {
        let items_block = Block::bordered().title("Items");
        match &self.monitors {
            Some(monitors) => {
                let rows: Vec<ListItem> = monitors.iter().map(|m| ListItem::new(m.row())).collect();
                f.render_widget(List::new(rows).block(items_block), items_area);
            }
            None => f.render_widget(Paragraph::new(UNAVAILABLE).block(items_block), items_area),
        }
        f.render_widget(Block::bordered().title("Preview"), preview_area);
    }
}

//! The Monitors items pane: monitors listed from `hyprctl monitors all -j`.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph};
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

/// What `Enter` on the monitor under the cursor should do.
pub(crate) enum Toggle {
    /// Run `ln-monitorctl <name> <action>`.
    Run { name: String, action: &'static str },
    /// The monitor is the last enabled one and must stay on.
    Refused,
}

pub(crate) struct MonitorsPane {
    /// `None` when the last query failed.
    monitors: Option<Vec<Monitor>>,
    /// Index into the monitor list of the highlighted row.
    cursor: usize,
}

impl MonitorsPane {
    /// An empty pane; nothing is queried until `load`.
    pub(crate) fn new() -> MonitorsPane {
        MonitorsPane {
            monitors: Some(Vec::new()),
            cursor: 0,
        }
    }

    /// Queries `hyprctl` and replaces the list, keeping the cursor on a valid row.
    pub(crate) fn load(&mut self, sys: &dyn System) {
        self.monitors = query(sys);
        let len = self.monitors.as_ref().map_or(0, Vec::len);
        self.cursor = self.cursor.min(len.saturating_sub(1));
    }

    /// Moves the cursor by `delta` rows, clamped to the first and last monitor.
    pub(crate) fn move_cursor(&mut self, delta: isize) {
        let len = self.monitors.as_ref().map_or(0, Vec::len);
        self.cursor = self
            .cursor
            .saturating_add_signed(delta)
            .min(len.saturating_sub(1));
    }

    /// The toggle for the monitor under the cursor; `None` when there is none.
    pub(crate) fn toggle(&self) -> Option<Toggle> {
        let monitors = self.monitors.as_ref()?;
        let m = monitors.get(self.cursor)?;
        if m.disabled {
            return Some(Toggle::Run {
                name: m.name.clone(),
                action: "on",
            });
        }
        if monitors.iter().filter(|m| !m.disabled).count() <= 1 {
            return Some(Toggle::Refused);
        }
        Some(Toggle::Run {
            name: m.name.clone(),
            action: "off",
        })
    }

    /// Draws the pane's two columns; the preview is empty.
    pub(crate) fn render(
        &self,
        f: &mut Frame,
        items_area: Rect,
        preview_area: Rect,
        highlight: Style,
    ) {
        let items_block = Block::bordered().title("Items");
        match &self.monitors {
            Some(monitors) => {
                let rows: Vec<ListItem> = monitors.iter().map(|m| ListItem::new(m.row())).collect();
                let list = List::new(rows)
                    .block(items_block)
                    .highlight_style(highlight);
                let mut state = ListState::default().with_selected(Some(self.cursor));
                f.render_stateful_widget(list, items_area, &mut state);
            }
            None => f.render_widget(Paragraph::new(UNAVAILABLE).block(items_block), items_area),
        }
        f.render_widget(Block::bordered().title("Preview"), preview_area);
    }
}

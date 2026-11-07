//! The Monitors items pane: monitors listed from `hyprctl monitors all -j`.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph};
use serde::Deserialize;

use crate::pane::{Action, Cursor, PaneAction, Program};
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

/// The state `ln-monitorctl` puts a monitor in.
#[derive(Clone, Copy)]
enum Power {
    On,
    Off,
}

impl Power {
    fn as_str(self) -> &'static str {
        match self {
            Power::On => "on",
            Power::Off => "off",
        }
    }
}

/// An open confirmation popup: the text lines and what `Enter` does (`None`: nothing).
pub(crate) struct Popup {
    pub(crate) lines: Vec<String>,
    pub(crate) confirm: Option<PaneAction>,
}

pub(crate) struct MonitorsPane {
    /// `None` when the last query failed.
    monitors: Option<Vec<Monitor>>,
    /// Index into the monitor list of the highlighted row.
    cursor: Cursor,
}

impl MonitorsPane {
    /// An empty pane; nothing is queried until `load`.
    pub(crate) fn new() -> MonitorsPane {
        MonitorsPane {
            monitors: Some(Vec::new()),
            cursor: Cursor::default(),
        }
    }

    /// Queries `hyprctl` and replaces the list, keeping the cursor on a valid row.
    pub(crate) fn load(&mut self, sys: &dyn System) {
        self.monitors = query(sys);
        self.cursor.clamp(self.len());
    }

    fn len(&self) -> usize {
        self.monitors.as_ref().map_or(0, Vec::len)
    }

    /// Handles a key while the Monitors items are focused.
    pub(crate) fn handle(&mut self, action: Action) -> Option<PaneAction> {
        self.cursor.navigate(action, self.len());
        None
    }

    /// Nothing to reset when focus leaves the pane.
    pub(crate) fn unfocus(&mut self) {}

    /// The confirmation popup for the monitor under the cursor, decided now: the switch to
    /// perform, the refusal for the last enabled monitor, or the notice that a job is running.
    pub(crate) fn popup(&self, busy: bool) -> Option<Popup> {
        let monitors = self.monitors.as_ref()?;
        let m = monitors.get(self.cursor.get())?;
        let (power, last_enabled) = match m.disabled {
            true => (Power::On, false),
            false => (
                Power::Off,
                monitors.iter().filter(|m| !m.disabled).count() <= 1,
            ),
        };
        let switch = format!("{} {}", m.name, power.as_str());
        let (message, confirm) = if busy {
            ("Applying…".to_owned(), None)
        } else if last_enabled {
            (
                "Refusing to disable the last enabled monitor".to_owned(),
                None,
            )
        } else {
            (
                switch.clone(),
                Some(PaneAction::Spawn {
                    program: Program::Monitorctl,
                    args: vec![m.name.clone(), power.as_str().to_owned()],
                    name: switch,
                }),
            )
        };
        Some(Popup {
            lines: vec![m.row(), message],
            confirm,
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
                let mut state = ListState::default().with_selected(Some(self.cursor.get()));
                f.render_stateful_widget(list, items_area, &mut state);
            }
            None => f.render_widget(Paragraph::new(UNAVAILABLE).block(items_block), items_area),
        }
        f.render_widget(Block::bordered().title("Preview"), preview_area);
    }
}

//! The Monitors popup: confirm, refusal or busy notice over the Monitors pane.

use ratatui::crossterm::event::KeyCode;

use crate::pane::{Action, SpawnJob};

/// What the popup offers for the row under the cursor.
pub(crate) enum PopupKind {
    /// `Enter` carries out the action.
    Confirm(SpawnJob),
    /// The switch is not allowed; the popup only explains why.
    Refuse,
    /// A job is already running.
    Busy,
}

/// An open popup over a monitor row: a confirmation, a refusal or the busy notice, as `kind` says.
pub(crate) struct Popup {
    row: String,
    kind: PopupKind,
}

/// What a key does to an open popup.
pub(crate) enum PopupNext {
    Keep(Popup),
    Close,
    Confirm(SpawnJob),
}

impl Popup {
    /// A popup over the monitor `row`, offering what `kind` says.
    pub(crate) fn new(row: String, kind: PopupKind) -> Popup {
        Popup { row, kind }
    }

    /// The same row, but a job is running so `Enter` does nothing.
    pub(crate) fn busy(self) -> Popup {
        Popup {
            kind: PopupKind::Busy,
            ..self
        }
    }

    /// The text shown: the row, then the switch name, the refusal or `Applying…`.
    pub(crate) fn lines(&self) -> Vec<String> {
        let message = match &self.kind {
            PopupKind::Confirm(job) => job.name.clone(),
            PopupKind::Refuse => "Refusing to disable the last enabled monitor".to_owned(),
            PopupKind::Busy => "Applying…".to_owned(),
        };
        vec![self.row.clone(), message]
    }

    /// Routes a key: `Esc` and the quit key close, `Enter` confirms a `Confirm` popup, anything
    /// else keeps it open.
    pub(crate) fn on_key(self, code: KeyCode, quit_key: char) -> PopupNext {
        match (Action::from_key(code, quit_key), self.kind) {
            (Some(Action::Cancel | Action::Quit), _) => PopupNext::Close,
            (Some(Action::Apply), PopupKind::Confirm(job)) => PopupNext::Confirm(job),
            (_, kind) => PopupNext::Keep(Popup { kind, ..self }),
        }
    }
}

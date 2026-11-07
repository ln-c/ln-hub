//! The Monitors popup: confirm, refusal or busy notice over the Monitors pane.

use ratatui::crossterm::event::KeyCode;

use crate::pane::{Action, PaneAction};

/// What the popup offers for the row under the cursor.
pub(crate) enum PopupKind {
    /// `Enter` carries out the action.
    Confirm(PaneAction),
    /// The switch is not allowed; the popup only explains why.
    Refuse,
    /// A job is already running.
    Busy,
}

/// An open confirmation popup: the monitor row and what `Enter` may do about it.
pub(crate) struct Popup {
    pub(crate) row: String,
    pub(crate) kind: PopupKind,
}

/// What a key does to an open popup.
pub(crate) enum PopupNext {
    Keep(Popup),
    Close,
    Confirm(PaneAction),
}

impl Popup {
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
            PopupKind::Confirm(PaneAction::Spawn { name, .. }) => name.clone(),
            PopupKind::Confirm(PaneAction::Status(message)) => message.clone(),
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
            (Some(Action::Apply), PopupKind::Confirm(action)) => PopupNext::Confirm(action),
            (_, kind) => PopupNext::Keep(Popup { kind, ..self }),
        }
    }
}

//! What the app and the items panes say to each other: resolved keys, requested jobs, and the
//! row cursor both panes share.

use ratatui::crossterm::event::KeyCode;

/// A key resolved to what it means outside a prompt.
#[derive(Clone, Copy)]
pub(crate) enum Action {
    Quit,
    Down,
    Up,
    Left,
    Right,
    Apply,
    FontName,
    FontSize,
}

/// The built-in character keys. `quit` may not take any of these (nor [`FILTER_KEY`]).
const CHAR_KEYS: [(char, Action); 6] = [
    ('j', Action::Down),
    ('k', Action::Up),
    ('h', Action::Left),
    ('l', Action::Right),
    ('f', Action::FontName),
    ('s', Action::FontSize),
];

/// Reserved for the items filter, which is not bound yet.
const FILTER_KEY: char = '/';

impl Action {
    pub(crate) fn from_key(code: KeyCode, quit_key: char) -> Option<Action> {
        match code {
            KeyCode::Char(c) if c == quit_key => Some(Action::Quit),
            KeyCode::Char(c) => CHAR_KEYS.iter().find(|(k, _)| *k == c).map(|(_, a)| *a),
            KeyCode::Down => Some(Action::Down),
            KeyCode::Up => Some(Action::Up),
            KeyCode::Left => Some(Action::Left),
            KeyCode::Right => Some(Action::Right),
            KeyCode::Enter => Some(Action::Apply),
            _ => None,
        }
    }

    /// Whether `key` is already bound (or reserved) and so cannot be the quit key.
    pub(crate) fn is_built_in_key(key: char) -> bool {
        key == FILTER_KEY || CHAR_KEYS.iter().any(|(k, _)| *k == key)
    }
}

/// What a pane asks the app to do after a key.
pub(crate) enum PaneAction {
    /// Spawn the pane's program with `args` as the in-flight job, labelled `name`.
    Spawn { args: Vec<String>, name: String },
    /// Show `message` in the status bar.
    Status(String),
}

/// The highlighted row of a list, kept inside the list.
#[derive(Default)]
pub(crate) struct Cursor(usize);

impl Cursor {
    pub(crate) fn at(row: usize) -> Cursor {
        Cursor(row)
    }

    pub(crate) fn get(&self) -> usize {
        self.0
    }

    /// Moves by `delta` rows within a list of `len` rows; does nothing when it is empty.
    pub(crate) fn move_by(&mut self, delta: isize, len: usize) {
        if let Some(last) = len.checked_sub(1) {
            self.0 = self.0.saturating_add_signed(delta).min(last);
        }
    }

    /// Pulls the cursor back onto the last row after the list shrank to `len` rows.
    pub(crate) fn clamp(&mut self, len: usize) {
        self.0 = self.0.min(len.saturating_sub(1));
    }
}

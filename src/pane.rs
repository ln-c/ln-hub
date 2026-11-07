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
    Filter,
    Cancel,
    Top,
    Bottom,
    Help,
}

/// The built-in character keys. `quit` may not take any of these (nor [`FILTER_KEY`]).
const CHAR_KEYS: [(char, Action); 8] = [
    ('j', Action::Down),
    ('k', Action::Up),
    ('h', Action::Left),
    ('l', Action::Right),
    ('f', Action::FontName),
    ('s', Action::FontSize),
    ('G', Action::Bottom),
    ('?', Action::Help),
];

/// Opens the Theme items filter.
const FILTER_KEY: char = '/';

/// Starts the `gg` (top) sequence; reserved from `quit` like the built-in keys.
pub(crate) const TOP_PREFIX: char = 'g';

impl Action {
    /// Resolves a key outside a prompt. The quit key is checked first; config validation keeps it
    /// disjoint from the built-in keys, so the order never shadows a binding.
    pub(crate) fn from_key(code: KeyCode, quit_key: char) -> Option<Action> {
        match code {
            KeyCode::Char(c) if c == quit_key => Some(Action::Quit),
            KeyCode::Char(FILTER_KEY) => Some(Action::Filter),
            KeyCode::Esc => Some(Action::Cancel),
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
        key == FILTER_KEY || key == TOP_PREFIX || CHAR_KEYS.iter().any(|(k, _)| *k == key)
    }
}

/// The helper program a job runs.
#[derive(Clone, Copy)]
pub(crate) enum Program {
    Themectl,
    Monitorctl,
}

/// What a pane asks the app to do after a key.
pub(crate) enum PaneAction {
    /// Spawn `program` with `args` as the in-flight job, labelled `name`.
    Spawn {
        program: Program,
        args: Vec<String>,
        name: String,
    },
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

    /// Applies a movement action (Down, Up, Top, Bottom) to a list of `len` rows; returns whether
    /// `action` was one.
    pub(crate) fn navigate(&mut self, action: Action, len: usize) -> bool {
        match action {
            Action::Down => self.move_by(1, len),
            Action::Up => self.move_by(-1, len),
            Action::Top => self.0 = 0,
            Action::Bottom => self.last(len),
            _ => return false,
        }
        true
    }

    /// Moves to the last row of a list of `len` rows; does nothing when it is empty.
    fn last(&mut self, len: usize) {
        self.0 = len.saturating_sub(1);
    }

    /// Pulls the cursor back onto the last row after the list shrank to `len` rows.
    pub(crate) fn clamp(&mut self, len: usize) {
        self.0 = self.0.min(len.saturating_sub(1));
    }
}

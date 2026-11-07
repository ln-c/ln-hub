//! Shared visual styles, kept in a leaf module so every pane can use them without depending on `app`.

use ratatui::style::{Modifier, Style};

/// Highlight for the selected row of any list.
pub(crate) const HIGHLIGHT: Style = Style::new().add_modifier(Modifier::REVERSED);

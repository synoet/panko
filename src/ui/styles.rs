//! Color scheme matching GitHub's dark mode diff view.
//! Uses basic terminal colors for maximum compatibility.

use ratatui::style::{Color, Modifier, Style};

// Use basic terminal colors for compatibility
pub const BG_DEFAULT: Color = Color::Reset;
pub const BG_SIDEBAR: Color = Color::Black;
pub const BG_HEADER: Color = Color::DarkGray;
pub const BG_SELECTED: Color = Color::DarkGray;
pub const BG_ADDITION: Color = Color::Black;
pub const BG_DELETION: Color = Color::Black;
pub const BG_HUNK_HEADER: Color = Color::Black;

pub const FG_DEFAULT: Color = Color::White;
pub const FG_MUTED: Color = Color::Gray;
pub const FG_ADDITION: Color = Color::Green;
pub const FG_DELETION: Color = Color::Red;
pub const FG_LINE_NUM: Color = Color::DarkGray;
pub const FG_HUNK: Color = Color::Magenta;
pub const FG_PATH: Color = Color::Cyan;
pub const FG_DIRECTORY: Color = Color::Blue;

pub fn style_default() -> Style {
    Style::default().fg(FG_DEFAULT)
}

pub fn style_muted() -> Style {
    Style::default().fg(FG_MUTED)
}

pub fn style_addition() -> Style {
    Style::default().fg(FG_ADDITION)
}

pub fn style_deletion() -> Style {
    Style::default().fg(FG_DELETION)
}

pub fn style_addition_line_num() -> Style {
    Style::default().fg(FG_LINE_NUM)
}

pub fn style_deletion_line_num() -> Style {
    Style::default().fg(FG_LINE_NUM)
}

pub fn style_context() -> Style {
    Style::default().fg(FG_DEFAULT)
}

pub fn style_line_num() -> Style {
    Style::default().fg(FG_LINE_NUM)
}

pub fn style_hunk_header() -> Style {
    Style::default().fg(FG_HUNK)
}

pub fn style_file_header() -> Style {
    Style::default().fg(FG_PATH).add_modifier(Modifier::BOLD)
}

pub fn style_selected() -> Style {
    Style::default().bg(BG_SELECTED)
}

pub fn style_directory() -> Style {
    Style::default().fg(FG_DIRECTORY)
}

pub fn style_stat_addition() -> Style {
    Style::default().fg(FG_ADDITION).add_modifier(Modifier::BOLD)
}

pub fn style_stat_deletion() -> Style {
    Style::default().fg(FG_DELETION).add_modifier(Modifier::BOLD)
}

pub fn style_sidebar() -> Style {
    Style::default().fg(FG_DEFAULT)
}

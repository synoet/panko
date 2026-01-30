//! Color scheme matching GitHub's dark mode diff view.
//! Uses RGB colors for rich visual experience.

#![allow(dead_code)]

use ratatui::style::{Color, Modifier, Style};

// Background colors - GitHub Dark
pub const BG_DEFAULT: Color = Color::Rgb(36, 41, 46);      // #24292e - main background
pub const BG_SIDEBAR: Color = Color::Rgb(31, 36, 40);      // #1f2428 - sidebar
pub const BG_HEADER: Color = Color::Rgb(36, 41, 46);       // #24292e - header
pub const BG_FILE_HEADER: Color = Color::Rgb(36, 41, 46);  // #24292e - file header
pub const BG_SELECTED: Color = Color::Rgb(57, 65, 74);     // #39414a - selection
pub const BG_HOVER: Color = Color::Rgb(40, 46, 52);        // #282e34 - hover

// Diff backgrounds - user specified
// Margin (line number gutter)
pub const BG_ADDITION_MARGIN: Color = Color::Rgb(40, 67, 45);   // #28432D - green gutter
pub const BG_DELETION_MARGIN: Color = Color::Rgb(79, 41, 40);   // #4F2928 - red gutter
// Code content area (inline)
pub const BG_ADDITION_LINE: Color = Color::Rgb(24, 39, 33);     // #182721 - green line bg
pub const BG_DELETION_LINE: Color = Color::Rgb(36, 24, 29);     // #24181D - red line bg
// Word-level highlights - lighter overlay
pub const BG_ADDITION_WORD: Color = Color::Rgb(50, 82, 56);     // #325238 - green word
pub const BG_DELETION_WORD: Color = Color::Rgb(99, 51, 50);     // #633332 - red word
// Selection variants
pub const BG_ADDITION_SELECTED: Color = Color::Rgb(32, 52, 42); // #20342a - selected green
pub const BG_DELETION_SELECTED: Color = Color::Rgb(50, 34, 38); // #322226 - selected red
pub const BG_CONTEXT_SELECTED: Color = Color::Rgb(57, 65, 74);  // #39414a - selection gray
// Hunk header
pub const BG_HUNK_HEADER: Color = Color::Rgb(36, 41, 56);       // blue tint
pub const BG_HUNK_EXPAND: Color = Color::Rgb(45, 55, 75);       // expand button bg

// Foreground colors - GitHub Dark
pub const FG_DEFAULT: Color = Color::Rgb(225, 228, 232);   // #e1e4e8 - main text
pub const FG_MUTED: Color = Color::Rgb(149, 157, 165);     // #959da5 - muted text
pub const FG_ADDITION: Color = Color::Rgb(52, 208, 88);    // #34d058 - green text
pub const FG_DELETION: Color = Color::Rgb(234, 74, 90);    // #ea4a5a - red text
pub const FG_LINE_NUM: Color = Color::Rgb(106, 115, 125);  // #6a737d - line numbers
pub const FG_LINE_NUM_HIGHLIGHT: Color = Color::Rgb(225, 228, 232); // #e1e4e8
pub const FG_HUNK: Color = Color::Rgb(121, 184, 255);      // #79b8ff - hunk info (blue)
pub const FG_PATH: Color = Color::Rgb(121, 184, 255);      // #79b8ff - file paths
pub const FG_DIRECTORY: Color = Color::Rgb(121, 184, 255); // #79b8ff - directories
pub const FG_BORDER: Color = Color::Rgb(68, 77, 86);       // #444d56 - borders
pub const FG_STATS_BAR: Color = Color::Rgb(149, 157, 165); // #959da5 - neutral stats
pub const FG_WARNING: Color = Color::Rgb(255, 171, 112);   // #ffab70 - orange warning
pub const FG_CURSOR: Color = Color::Rgb(249, 130, 108);    // #f9826c - cursor indicator

// Border characters (rounded)
pub const BORDER_TOP_LEFT: &str = "╭";
pub const BORDER_TOP_RIGHT: &str = "╮";
pub const BORDER_BOTTOM_LEFT: &str = "╰";
pub const BORDER_BOTTOM_RIGHT: &str = "╯";
pub const BORDER_HORIZONTAL: &str = "─";
pub const BORDER_VERTICAL: &str = "│";

// Style functions
pub fn style_default() -> Style {
    Style::default().fg(FG_DEFAULT).bg(BG_DEFAULT)
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

pub fn style_addition_line() -> Style {
    Style::default().fg(FG_DEFAULT).bg(BG_ADDITION_LINE)
}

pub fn style_deletion_line() -> Style {
    Style::default().fg(FG_DEFAULT).bg(BG_DELETION_LINE)
}

pub fn style_addition_word() -> Style {
    Style::default().fg(FG_DEFAULT).bg(BG_ADDITION_WORD)
}

pub fn style_deletion_word() -> Style {
    Style::default().fg(FG_DEFAULT).bg(BG_DELETION_WORD)
}

pub fn style_addition_line_num() -> Style {
    Style::default().fg(FG_LINE_NUM).bg(BG_ADDITION_LINE)
}

pub fn style_deletion_line_num() -> Style {
    Style::default().fg(FG_LINE_NUM).bg(BG_DELETION_LINE)
}

pub fn style_context() -> Style {
    Style::default().fg(FG_DEFAULT).bg(BG_DEFAULT)
}

pub fn style_line_num() -> Style {
    Style::default().fg(FG_LINE_NUM).bg(BG_DEFAULT)
}

pub fn style_hunk_header() -> Style {
    Style::default().fg(FG_HUNK).bg(BG_HUNK_HEADER)
}

pub fn style_file_header() -> Style {
    Style::default()
        .fg(FG_PATH)
        .bg(BG_FILE_HEADER)
        .add_modifier(Modifier::BOLD)
}

pub fn style_file_header_selected() -> Style {
    Style::default()
        .fg(FG_PATH)
        .bg(BG_SELECTED)
        .add_modifier(Modifier::BOLD)
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

pub fn style_border() -> Style {
    Style::default().fg(FG_BORDER)
}

pub fn style_border_selected() -> Style {
    Style::default().fg(FG_PATH)
}

pub fn style_sidebar() -> Style {
    Style::default().fg(FG_DEFAULT).bg(BG_SIDEBAR)
}

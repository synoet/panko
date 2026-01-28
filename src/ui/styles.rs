//! Color scheme matching GitHub's dark mode diff view.
//! Uses RGB colors for rich visual experience.

#![allow(dead_code)]

use ratatui::style::{Color, Modifier, Style};

// Background colors - GitHub dark mode inspired
pub const BG_DEFAULT: Color = Color::Rgb(13, 17, 23);      // #0d1117 - main background
pub const BG_SIDEBAR: Color = Color::Rgb(13, 17, 23);      // #0d1117 - same as main bg
pub const BG_HEADER: Color = Color::Rgb(22, 27, 34);       // #161b22 - header
pub const BG_FILE_HEADER: Color = Color::Rgb(22, 27, 34);  // #161b22 - file header
pub const BG_SELECTED: Color = Color::Rgb(48, 54, 61);     // #30363d - selection
pub const BG_HOVER: Color = Color::Rgb(33, 38, 45);        // #21262d - hover

// Diff backgrounds - GitHub style with brighter margins
// Margin (line number gutter) - more saturated
pub const BG_ADDITION_MARGIN: Color = Color::Rgb(36, 71, 51);  // #244733 - green margin
pub const BG_DELETION_MARGIN: Color = Color::Rgb(82, 39, 42);  // #52272a - red margin
// Code content area - more subtle
pub const BG_ADDITION_LINE: Color = Color::Rgb(21, 43, 31);    // #152b1f - green line bg
pub const BG_DELETION_LINE: Color = Color::Rgb(52, 27, 31);    // #341b1f - red line bg
// Word-level highlights - brightest
pub const BG_ADDITION_WORD: Color = Color::Rgb(38, 109, 58);   // #266d3a - green word
pub const BG_DELETION_WORD: Color = Color::Rgb(139, 40, 45);   // #8b282d - red word
// Selection variants - brighter versions for when lines are selected
pub const BG_ADDITION_SELECTED: Color = Color::Rgb(36, 71, 51);  // #244733 - brighter green
pub const BG_DELETION_SELECTED: Color = Color::Rgb(82, 50, 55);  // #523237 - brighter red
pub const BG_CONTEXT_SELECTED: Color = Color::Rgb(48, 54, 61);   // #30363d - selection gray
// Hunk header
pub const BG_HUNK_HEADER: Color = Color::Rgb(22, 27, 46);      // #161b2e - blue hunk bg
pub const BG_HUNK_EXPAND: Color = Color::Rgb(31, 41, 63);      // #1f293f - expand button bg

// Foreground colors
pub const FG_DEFAULT: Color = Color::Rgb(230, 237, 243);   // #e6edf3 - main text
pub const FG_MUTED: Color = Color::Rgb(125, 133, 144);     // #7d8590 - muted text
pub const FG_ADDITION: Color = Color::Rgb(63, 185, 80);    // #3fb950 - green text
pub const FG_DELETION: Color = Color::Rgb(248, 81, 73);    // #f85149 - red text
pub const FG_LINE_NUM: Color = Color::Rgb(110, 118, 129);  // #6e7681 - line numbers
pub const FG_LINE_NUM_HIGHLIGHT: Color = Color::Rgb(201, 209, 217); // #c9d1d9 - highlighted line num
pub const FG_HUNK: Color = Color::Rgb(121, 192, 255);      // #79c0ff - hunk info (blue)
pub const FG_PATH: Color = Color::Rgb(121, 192, 255);      // #79c0ff - file paths
pub const FG_DIRECTORY: Color = Color::Rgb(121, 192, 255); // #79c0ff - directories
pub const FG_BORDER: Color = Color::Rgb(48, 54, 61);       // #30363d - borders
pub const FG_STATS_BAR: Color = Color::Rgb(155, 155, 155); // #9b9b9b - neutral stats
pub const FG_WARNING: Color = Color::Rgb(227, 140, 56);    // #e38c38 - orange warning
pub const FG_CURSOR: Color = Color::Rgb(255, 213, 79);     // #ffd54f - yellow cursor indicator

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

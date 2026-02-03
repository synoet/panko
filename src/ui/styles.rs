//! Theme-aware style helpers.

#![allow(dead_code)]

use ratatui::style::{Color, Modifier, Style};

use crate::ui::theme;

pub fn ui() -> theme::UiTheme {
    theme::current_ui()
}

// Background colors
pub fn bg_default() -> Color {
    theme::current_ui().bg_default
}

pub fn bg_sidebar() -> Color {
    theme::current_ui().bg_sidebar
}

pub fn bg_header() -> Color {
    theme::current_ui().bg_header
}

pub fn bg_file_header() -> Color {
    theme::current_ui().bg_file_header
}

pub fn bg_selected() -> Color {
    theme::current_ui().bg_selected
}

pub fn bg_hover() -> Color {
    theme::current_ui().bg_hover
}

pub fn bg_addition_margin() -> Color {
    theme::current_ui().bg_addition_margin
}

pub fn bg_deletion_margin() -> Color {
    theme::current_ui().bg_deletion_margin
}

pub fn bg_addition_line() -> Color {
    theme::current_ui().bg_addition_line
}

pub fn bg_deletion_line() -> Color {
    theme::current_ui().bg_deletion_line
}

pub fn bg_addition_word() -> Color {
    theme::current_ui().bg_addition_word
}

pub fn bg_deletion_word() -> Color {
    theme::current_ui().bg_deletion_word
}

pub fn bg_addition_selected() -> Color {
    theme::current_ui().bg_addition_selected
}

pub fn bg_deletion_selected() -> Color {
    theme::current_ui().bg_deletion_selected
}

pub fn bg_context_selected() -> Color {
    theme::current_ui().bg_context_selected
}

pub fn bg_hunk_header() -> Color {
    theme::current_ui().bg_hunk_header
}

pub fn bg_hunk_expand() -> Color {
    theme::current_ui().bg_hunk_expand
}

// Foreground colors
pub fn fg_default() -> Color {
    theme::current_ui().fg_default
}

pub fn fg_muted() -> Color {
    theme::current_ui().fg_muted
}

pub fn fg_addition() -> Color {
    theme::current_ui().fg_addition
}

pub fn fg_deletion() -> Color {
    theme::current_ui().fg_deletion
}

pub fn fg_line_num() -> Color {
    theme::current_ui().fg_line_num
}

pub fn fg_line_num_highlight() -> Color {
    theme::current_ui().fg_line_num_highlight
}

pub fn fg_hunk() -> Color {
    theme::current_ui().fg_hunk
}

pub fn fg_path() -> Color {
    theme::current_ui().fg_path
}

pub fn fg_directory() -> Color {
    theme::current_ui().fg_directory
}

pub fn fg_border() -> Color {
    theme::current_ui().fg_border
}

pub fn fg_stats_bar() -> Color {
    theme::current_ui().fg_stats_bar
}

pub fn fg_warning() -> Color {
    theme::current_ui().fg_warning
}

pub fn fg_cursor() -> Color {
    theme::current_ui().fg_cursor
}

// Border characters
pub fn border_top_left() -> &'static str {
    theme::current_ui().border_top_left
}

pub fn border_top_right() -> &'static str {
    theme::current_ui().border_top_right
}

pub fn border_bottom_left() -> &'static str {
    theme::current_ui().border_bottom_left
}

pub fn border_bottom_right() -> &'static str {
    theme::current_ui().border_bottom_right
}

pub fn border_horizontal() -> &'static str {
    theme::current_ui().border_horizontal
}

pub fn border_vertical() -> &'static str {
    theme::current_ui().border_vertical
}

// Style functions
pub fn style_default() -> Style {
    Style::default().fg(fg_default()).bg(bg_default())
}

pub fn style_muted() -> Style {
    Style::default().fg(fg_muted())
}

pub fn style_addition() -> Style {
    Style::default().fg(fg_addition())
}

pub fn style_deletion() -> Style {
    Style::default().fg(fg_deletion())
}

pub fn style_addition_line() -> Style {
    Style::default().fg(fg_default()).bg(bg_addition_line())
}

pub fn style_deletion_line() -> Style {
    Style::default().fg(fg_default()).bg(bg_deletion_line())
}

pub fn style_addition_word() -> Style {
    Style::default().fg(fg_default()).bg(bg_addition_word())
}

pub fn style_deletion_word() -> Style {
    Style::default().fg(fg_default()).bg(bg_deletion_word())
}

pub fn style_addition_line_num() -> Style {
    Style::default().fg(fg_line_num()).bg(bg_addition_line())
}

pub fn style_deletion_line_num() -> Style {
    Style::default().fg(fg_line_num()).bg(bg_deletion_line())
}

pub fn style_context() -> Style {
    Style::default().fg(fg_default()).bg(bg_default())
}

pub fn style_line_num() -> Style {
    Style::default().fg(fg_line_num()).bg(bg_default())
}

pub fn style_hunk_header() -> Style {
    Style::default().fg(fg_hunk()).bg(bg_hunk_header())
}

pub fn style_file_header() -> Style {
    Style::default()
        .fg(fg_path())
        .bg(bg_file_header())
        .add_modifier(Modifier::BOLD)
}

pub fn style_file_header_selected() -> Style {
    Style::default()
        .fg(fg_path())
        .bg(bg_selected())
        .add_modifier(Modifier::BOLD)
}

pub fn style_selected() -> Style {
    Style::default().bg(bg_selected())
}

pub fn style_directory() -> Style {
    Style::default().fg(fg_directory())
}

pub fn style_stat_addition() -> Style {
    Style::default().fg(fg_addition()).add_modifier(Modifier::BOLD)
}

pub fn style_stat_deletion() -> Style {
    Style::default().fg(fg_deletion()).add_modifier(Modifier::BOLD)
}

pub fn style_border() -> Style {
    Style::default().fg(fg_border())
}

pub fn style_border_selected() -> Style {
    Style::default().fg(fg_path())
}

pub fn style_sidebar() -> Style {
    Style::default().fg(fg_default()).bg(bg_sidebar())
}

//! Syntax highlighting using two-face and the active theme.

#![allow(dead_code)]

use once_cell::sync::Lazy;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use syntect::easy::HighlightLines;
use syntect::highlighting::FontStyle;
use syntect::parsing::SyntaxSet;

use crate::ui::theme;

static SYNTAX_SET: Lazy<SyntaxSet> = Lazy::new(two_face::syntax::extra_newlines);

/// Get syntax-highlighted spans for a line of code.
pub fn highlight_line(content: &str, extension: &str) -> Vec<Span<'static>> {
    let syntax = SYNTAX_SET
        .find_syntax_by_extension(extension)
        .or_else(|| SYNTAX_SET.find_syntax_by_extension("txt"))
        .unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text());

    let spans = theme::with_syntax_theme(|syntect_theme| {
        let mut highlighter = HighlightLines::new(syntax, syntect_theme);
        highlighter.highlight_line(content, &SYNTAX_SET)
    });

    match spans {
        Ok(ranges) => ranges
            .into_iter()
            .map(|(style, text)| {
                let fg = Color::Rgb(
                    style.foreground.r,
                    style.foreground.g,
                    style.foreground.b,
                );
                let mut ratatui_style = Style::default().fg(fg);

                if style.font_style.contains(FontStyle::BOLD) {
                    ratatui_style = ratatui_style.add_modifier(Modifier::BOLD);
                }
                if style.font_style.contains(FontStyle::ITALIC) {
                    ratatui_style = ratatui_style.add_modifier(Modifier::ITALIC);
                }

                Span::styled(text.to_string(), ratatui_style)
            })
            .collect(),
        Err(_) => vec![Span::raw(content.to_string())],
    }
}

/// Extract file extension from path.
pub fn get_extension(path: &str) -> &str {
    path.rsplit('.').next().unwrap_or("txt")
}

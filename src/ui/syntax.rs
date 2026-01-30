//! Syntax highlighting using two-face with GitHub Dark theme.

#![allow(dead_code)]

use std::str::FromStr;

use once_cell::sync::Lazy;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use syntect::easy::HighlightLines;
use syntect::highlighting::{
    Color as SynColor, FontStyle, ScopeSelectors, StyleModifier, Theme, ThemeItem, ThemeSettings,
};
use syntect::parsing::SyntaxSet;

static SYNTAX_SET: Lazy<SyntaxSet> = Lazy::new(two_face::syntax::extra_newlines);
static THEME: Lazy<Theme> = Lazy::new(github_dark_theme);

/// GitHub Dark Classic theme colors
fn github_dark_theme() -> Theme {
    let fg = SynColor { r: 225, g: 228, b: 232, a: 255 };       // #e1e4e8
    let comment = SynColor { r: 106, g: 115, b: 125, a: 255 };  // #6a737d
    let keyword = SynColor { r: 249, g: 117, b: 131, a: 255 };  // #f97583
    let string = SynColor { r: 158, g: 203, b: 255, a: 255 };   // #9ecbff
    let constant = SynColor { r: 121, g: 184, b: 255, a: 255 }; // #79b8ff
    let entity = SynColor { r: 179, g: 146, b: 240, a: 255 };   // #b392f0
    let tag = SynColor { r: 133, g: 232, b: 157, a: 255 };      // #85e89d
    let variable = SynColor { r: 255, g: 171, b: 112, a: 255 }; // #ffab70

    Theme {
        name: Some("GitHub Dark".into()),
        author: Some("GitHub".into()),
        settings: ThemeSettings {
            foreground: Some(fg),
            background: Some(SynColor { r: 36, g: 41, b: 46, a: 255 }), // #24292e
            ..Default::default()
        },
        scopes: vec![
            // Comments
            theme_item("comment", comment, false, true),
            theme_item("punctuation.definition.comment", comment, false, true),
            // Keywords (if, else, return, etc.)
            theme_item("keyword", keyword, false, false),
            theme_item("keyword.control", keyword, false, false),
            theme_item("keyword.operator", keyword, false, false),
            // Storage (let, const, fn, var, function, class, etc.)
            theme_item("storage", keyword, false, false),
            theme_item("storage.type", keyword, false, false),
            theme_item("storage.modifier", keyword, false, false),
            // Strings
            theme_item("string", string, false, false),
            theme_item("punctuation.definition.string", string, false, false),
            // Constants and numbers
            theme_item("constant", constant, false, false),
            theme_item("constant.numeric", constant, false, false),
            theme_item("constant.language", constant, false, false),
            theme_item("variable.other.constant", constant, false, false),
            // Functions and methods
            theme_item("entity.name.function", entity, false, false),
            theme_item("entity.name.method", entity, false, false),
            theme_item("support.function", entity, false, false),
            theme_item("meta.function-call", entity, false, false),
            // Types and classes
            theme_item("entity.name.type", entity, false, false),
            theme_item("entity.name.class", entity, false, false),
            theme_item("support.type", constant, false, false),
            theme_item("support.class", constant, false, false),
            // Tags (HTML/JSX)
            theme_item("entity.name.tag", tag, false, false),
            // Variables
            theme_item("variable", variable, false, false),
            theme_item("variable.parameter", fg, false, false),
            theme_item("variable.other", fg, false, false),
            // Properties
            theme_item("meta.property-name", constant, false, false),
            theme_item("support.variable.property", constant, false, false),
            // Punctuation (keep it subtle)
            theme_item("punctuation", fg, false, false),
            // Operators
            theme_item("keyword.operator", fg, false, false),
        ],
        ..Default::default()
    }
}

fn theme_item(scope: &str, color: SynColor, bold: bool, italic: bool) -> ThemeItem {
    let mut font_style = FontStyle::empty();
    if bold {
        font_style |= FontStyle::BOLD;
    }
    if italic {
        font_style |= FontStyle::ITALIC;
    }
    ThemeItem {
        scope: ScopeSelectors::from_str(scope).unwrap_or_default(),
        style: StyleModifier {
            foreground: Some(color),
            font_style: Some(font_style),
            ..Default::default()
        },
    }
}

/// Get syntax-highlighted spans for a line of code.
pub fn highlight_line(content: &str, extension: &str) -> Vec<Span<'static>> {
    let syntax = SYNTAX_SET
        .find_syntax_by_extension(extension)
        .or_else(|| SYNTAX_SET.find_syntax_by_extension("txt"))
        .unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text());

    let mut highlighter = HighlightLines::new(syntax, &THEME);

    match highlighter.highlight_line(content, &SYNTAX_SET) {
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

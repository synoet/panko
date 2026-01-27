//! Syntax highlighting using syntect with Monokai theme (high contrast).

#![allow(dead_code)]

use once_cell::sync::Lazy;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, ThemeSet};
use syntect::parsing::SyntaxSet;

static SYNTAX_SET: Lazy<SyntaxSet> = Lazy::new(SyntaxSet::load_defaults_newlines);
static THEME_SET: Lazy<ThemeSet> = Lazy::new(ThemeSet::load_defaults);

/// Get syntax-highlighted spans for a line of code.
/// Uses InspiredGitHub theme for high contrast on dark backgrounds.
pub fn highlight_line(content: &str, extension: &str) -> Vec<Span<'static>> {
    let syntax = SYNTAX_SET
        .find_syntax_by_extension(extension)
        .or_else(|| SYNTAX_SET.find_syntax_by_extension("txt"))
        .unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text());

    // Use InspiredGitHub for better contrast (it has bright colors that work well on dark bg)
    let theme = &THEME_SET.themes["InspiredGitHub"];
    let mut highlighter = HighlightLines::new(syntax, theme);

    match highlighter.highlight_line(content, &SYNTAX_SET) {
        Ok(ranges) => ranges
            .into_iter()
            .map(|(style, text)| {
                // Boost brightness for dark background compatibility
                let (r, g, b) = boost_for_dark_bg(style.foreground.r, style.foreground.g, style.foreground.b);
                let fg = Color::Rgb(r, g, b);
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

/// Boost color brightness for visibility on dark backgrounds.
/// Maps light-theme colors to GitHub dark mode equivalents.
fn boost_for_dark_bg(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    // Calculate perceived luminance
    let lum = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;

    // If color is too dark for dark bg, brighten it while preserving hue
    if lum < 100.0 {
        // Map dark colors to GitHub dark mode palette
        // Dark blue -> bright blue
        if b > r && b > g {
            return (121, 192, 255); // #79c0ff - GitHub blue
        }
        // Dark green -> bright green
        if g > r && g > b {
            return (63, 185, 80); // #3fb950 - GitHub green
        }
        // Dark red/magenta -> coral
        if r > g {
            return (255, 123, 114); // #ff7b72 - GitHub red/coral
        }
        // Default: brighten to light gray
        return (230, 237, 243); // #e6edf3 - GitHub default text
    }

    // For already bright colors, tweak to GitHub palette
    // Bright blue
    if b > 200 && r < 150 && g < 200 {
        return (165, 214, 255); // #a5d6ff - light blue (strings)
    }
    // Purple/magenta
    if r > 150 && b > 150 && g < 150 {
        return (210, 168, 255); // #d2a8ff - purple (functions)
    }
    // Orange/yellow
    if r > 200 && g > 100 && b < 150 {
        return (255, 166, 87); // #ffa657 - orange (types)
    }
    // Green
    if g > 150 && r < 150 && b < 150 {
        return (126, 231, 135); // #7ee787 - bright green
    }

    // Keep as-is if already good contrast
    (r, g, b)
}

/// Extract file extension from path, with mappings for unsupported types.
pub fn get_extension(path: &str) -> &str {
    let ext = path.rsplit('.').next().unwrap_or("txt");

    // Map TypeScript and other extensions to supported syntaxes
    match ext {
        "ts" => "js",
        "tsx" => "jsx",
        "mts" => "js",
        "cts" => "js",
        "mjs" => "js",
        "cjs" => "js",
        "svelte" => "html",
        "vue" => "html",
        "astro" => "html",
        _ => ext,
    }
}

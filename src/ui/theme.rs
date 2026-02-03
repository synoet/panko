//! Theme registry and active theme state.

use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::RwLock;

use once_cell::sync::Lazy;
use ratatui::style::Color;
use syntect::highlighting::{
    Color as SynColor, FontStyle, ScopeSelectors, StyleModifier, Theme, ThemeItem, ThemeSettings,
};
use toml::Value as TomlValue;

#[derive(Debug, Clone, Copy)]
pub struct UiTheme {
    pub bg_default: Color,
    pub bg_sidebar: Color,
    pub bg_header: Color,
    pub bg_file_header: Color,
    pub bg_selected: Color,
    pub bg_hover: Color,
    pub bg_addition_margin: Color,
    pub bg_deletion_margin: Color,
    pub bg_addition_line: Color,
    pub bg_deletion_line: Color,
    pub bg_addition_word: Color,
    pub bg_deletion_word: Color,
    pub bg_addition_selected: Color,
    pub bg_deletion_selected: Color,
    pub bg_context_selected: Color,
    pub bg_hunk_header: Color,
    pub bg_hunk_expand: Color,
    pub fg_default: Color,
    pub fg_muted: Color,
    pub fg_addition: Color,
    pub fg_deletion: Color,
    pub fg_line_num: Color,
    pub fg_line_num_highlight: Color,
    pub fg_hunk: Color,
    pub fg_path: Color,
    pub fg_directory: Color,
    pub fg_border: Color,
    pub fg_stats_bar: Color,
    pub fg_warning: Color,
    pub fg_cursor: Color,
    pub border_top_left: &'static str,
    pub border_top_right: &'static str,
    pub border_bottom_left: &'static str,
    pub border_bottom_right: &'static str,
    pub border_horizontal: &'static str,
    pub border_vertical: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct SyntaxPalette {
    pub fg: SynColor,
    pub background: SynColor,
    pub comment: SynColor,
    pub keyword: SynColor,
    pub string: SynColor,
    pub constant: SynColor,
    pub entity: SynColor,
    pub tag: SynColor,
    pub variable: SynColor,
}

#[derive(Clone, Copy)]
pub struct ThemeSpec {
    pub name: &'static str,
    pub ui: UiTheme,
    pub syntax: SyntaxPalette,
}

struct ThemeState {
    name: String,
    ui: UiTheme,
    syntect_theme: Theme,
}

static THEME_STATE: Lazy<RwLock<ThemeState>> = Lazy::new(|| {
    let spec = github_dark();
    let syntect_theme = build_syntect_theme(&spec.syntax, spec.name);
    RwLock::new(ThemeState {
        name: spec.name.to_string(),
        ui: spec.ui,
        syntect_theme,
    })
});

const THEME_ORDER: &[&str] = &[
    "github-dark",
    "github-light",
    "catppuccin-mocha",
    "catppuccin-macchiato",
    "catppuccin-frappe",
    "catppuccin-latte",
];

pub fn available_themes() -> Vec<String> {
    THEME_ORDER.iter().map(|name| (*name).to_string()).collect()
}

pub fn build_theme_list() -> Vec<String> {
    available_themes()
}

pub fn current_name() -> String {
    THEME_STATE.read().expect("theme state").name.clone()
}

pub fn current_ui() -> UiTheme {
    THEME_STATE.read().expect("theme state").ui
}

pub fn with_syntax_theme<R>(f: impl FnOnce(&Theme) -> R) -> R {
    let guard = THEME_STATE.read().expect("theme state");
    f(&guard.syntect_theme)
}

pub fn load_theme_config() -> Option<String> {
    let path = theme_config_path().ok()?;
    let text = fs::read_to_string(path).ok()?;
    let value = text.parse::<TomlValue>().ok()?;
    let table = value.as_table()?;
    table.get("theme").and_then(|v| v.as_str()).map(|s| s.to_string())
}

pub fn save_theme_config(theme: &str) -> Result<(), String> {
    let path = theme_config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = format!("theme = \"{}\"\n", theme);
    fs::write(path, content).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn set_theme(name: &str) -> Result<(), String> {
    let spec = find_theme(name).ok_or_else(|| {
        format!(
            "Unknown theme '{}'. Available: {}",
            name,
            available_themes().join(", ")
        )
    })?;

    let syntect_theme = build_syntect_theme(&spec.syntax, spec.name);
    let mut guard = THEME_STATE.write().expect("theme state");
    guard.name = spec.name.to_string();
    guard.ui = spec.ui;
    guard.syntect_theme = syntect_theme;
    Ok(())
}

pub fn set_theme_and_persist(name: &str) -> Result<(), String> {
    set_theme(name)?;
    save_theme_config(name)?;
    Ok(())
}

pub fn init_from_env_and_arg(theme_arg: Option<&str>) -> Result<(), String> {
    if let Some(arg) = theme_arg {
        return set_theme(arg);
    }

    if let Ok(value) = std::env::var("PANKO_THEME") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return set_theme(trimmed);
        }
    }

    if let Some(saved) = load_theme_config() {
        return set_theme(&saved);
    }

    Ok(())
}

fn theme_config_path() -> Result<PathBuf, String> {
    let dir = dirs::config_dir().ok_or_else(|| "config dir not found".to_string())?;
    Ok(dir.join("panko").join("config.toml"))
}

fn normalize(name: &str) -> String {
    name.trim().to_lowercase().replace('_', "-")
}

fn find_theme(name: &str) -> Option<ThemeSpec> {
    match normalize(name).as_str() {
        "github-dark" | "dark" | "gh-dark" => Some(github_dark()),
        "github-light" | "light" | "gh-light" => Some(github_light()),
        "catppuccin" | "catpuccin" | "catppuccin-dark" | "catpuccin-dark" | "catppuccin-mocha"
        | "catpuccin-mocha" => Some(catppuccin_mocha()),
        "catppuccin-macchiato" | "catpuccin-macchiato" => Some(catppuccin_macchiato()),
        "catppuccin-frappe" | "catpuccin-frappe" => Some(catppuccin_frappe()),
        "catppuccin-light" | "catpuccin-light" | "catppuccin-latte"
        | "catpuccin-latte" => Some(catppuccin_latte()),
        _ => None,
    }
}

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

fn syn(r: u8, g: u8, b: u8) -> SynColor {
    SynColor { r, g, b, a: 255 }
}

fn as_rgb(color: Color) -> (u8, u8, u8) {
    match color {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => (0, 0, 0),
    }
}

fn blend(a: Color, b: Color, t: f32) -> Color {
    let (ar, ag, ab) = as_rgb(a);
    let (br, bg, bb) = as_rgb(b);
    let mix = |a: u8, b: u8| ((a as f32) * (1.0 - t) + (b as f32) * t).round() as u8;
    rgb(mix(ar, br), mix(ag, bg), mix(ab, bb))
}

// ─── GitHub ──────────────────────────────────────────────────────────────────

fn github_dark() -> ThemeSpec {
    ThemeSpec {
        name: "github-dark",
        ui: UiTheme {
            bg_default: rgb(36, 41, 46),
            bg_sidebar: rgb(31, 36, 40),
            bg_header: rgb(36, 41, 46),
            bg_file_header: rgb(36, 41, 46),
            bg_selected: rgb(57, 65, 74),
            bg_hover: rgb(40, 46, 52),
            bg_addition_margin: rgb(40, 67, 45),
            bg_deletion_margin: rgb(79, 41, 40),
            bg_addition_line: rgb(24, 39, 33),
            bg_deletion_line: rgb(36, 24, 29),
            bg_addition_word: rgb(50, 82, 56),
            bg_deletion_word: rgb(99, 51, 50),
            bg_addition_selected: rgb(32, 52, 42),
            bg_deletion_selected: rgb(50, 34, 38),
            bg_context_selected: rgb(57, 65, 74),
            bg_hunk_header: rgb(36, 41, 56),
            bg_hunk_expand: rgb(45, 55, 75),
            fg_default: rgb(225, 228, 232),
            fg_muted: rgb(149, 157, 165),
            fg_addition: rgb(52, 208, 88),
            fg_deletion: rgb(234, 74, 90),
            fg_line_num: rgb(106, 115, 125),
            fg_line_num_highlight: rgb(225, 228, 232),
            fg_hunk: rgb(121, 184, 255),
            fg_path: rgb(121, 184, 255),
            fg_directory: rgb(121, 184, 255),
            fg_border: rgb(68, 77, 86),
            fg_stats_bar: rgb(149, 157, 165),
            fg_warning: rgb(255, 171, 112),
            fg_cursor: rgb(249, 130, 108),
            border_top_left: "╭",
            border_top_right: "╮",
            border_bottom_left: "╰",
            border_bottom_right: "╯",
            border_horizontal: "─",
            border_vertical: "│",
        },
        syntax: SyntaxPalette {
            fg: syn(225, 228, 232),
            background: syn(36, 41, 46),
            comment: syn(106, 115, 125),
            keyword: syn(249, 117, 131),
            string: syn(158, 203, 255),
            constant: syn(121, 184, 255),
            entity: syn(179, 146, 240),
            tag: syn(133, 232, 157),
            variable: syn(255, 171, 112),
        },
    }
}

fn github_light() -> ThemeSpec {
    ThemeSpec {
        name: "github-light",
        ui: UiTheme {
            bg_default: rgb(255, 255, 255),
            bg_sidebar: rgb(246, 248, 250),
            bg_header: rgb(255, 255, 255),
            bg_file_header: rgb(255, 255, 255),
            bg_selected: rgb(234, 238, 242),
            bg_hover: rgb(243, 244, 246),
            bg_addition_margin: rgb(216, 248, 225),
            bg_deletion_margin: rgb(255, 235, 233),
            bg_addition_line: rgb(230, 255, 237),
            bg_deletion_line: rgb(255, 238, 240),
            bg_addition_word: rgb(172, 242, 189),
            bg_deletion_word: rgb(253, 184, 192),
            bg_addition_selected: rgb(205, 240, 218),
            bg_deletion_selected: rgb(255, 220, 225),
            bg_context_selected: rgb(234, 238, 242),
            bg_hunk_header: rgb(221, 244, 255),
            bg_hunk_expand: rgb(204, 233, 255),
            fg_default: rgb(36, 41, 47),
            fg_muted: rgb(87, 96, 106),
            fg_addition: rgb(26, 127, 55),
            fg_deletion: rgb(207, 34, 46),
            fg_line_num: rgb(110, 119, 129),
            fg_line_num_highlight: rgb(36, 41, 47),
            fg_hunk: rgb(9, 105, 218),
            fg_path: rgb(9, 105, 218),
            fg_directory: rgb(9, 105, 218),
            fg_border: rgb(208, 215, 222),
            fg_stats_bar: rgb(87, 96, 106),
            fg_warning: rgb(154, 103, 0),
            fg_cursor: rgb(188, 76, 0),
            border_top_left: "╭",
            border_top_right: "╮",
            border_bottom_left: "╰",
            border_bottom_right: "╯",
            border_horizontal: "─",
            border_vertical: "│",
        },
        syntax: SyntaxPalette {
            fg: syn(36, 41, 47),
            background: syn(255, 255, 255),
            comment: syn(110, 119, 129),
            keyword: syn(207, 34, 46),
            string: syn(10, 48, 105),
            constant: syn(5, 80, 174),
            entity: syn(130, 80, 223),
            tag: syn(17, 99, 41),
            variable: syn(149, 56, 0),
        },
    }
}

// ─── Catppuccin ──────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
struct CatppuccinPalette {
    base: (u8, u8, u8),
    mantle: (u8, u8, u8),
    surface0: (u8, u8, u8),
    surface1: (u8, u8, u8),
    surface2: (u8, u8, u8),
    text: (u8, u8, u8),
    subtext0: (u8, u8, u8),
    overlay0: (u8, u8, u8),
    blue: (u8, u8, u8),
    lavender: (u8, u8, u8),
    mauve: (u8, u8, u8),
    green: (u8, u8, u8),
    yellow: (u8, u8, u8),
    peach: (u8, u8, u8),
    red: (u8, u8, u8),
}

fn catppuccin_theme(name: &'static str, p: CatppuccinPalette, is_light: bool) -> ThemeSpec {
    let base = rgb(p.base.0, p.base.1, p.base.2);
    let mantle = rgb(p.mantle.0, p.mantle.1, p.mantle.2);
    let surface0 = rgb(p.surface0.0, p.surface0.1, p.surface0.2);
    let surface1 = rgb(p.surface1.0, p.surface1.1, p.surface1.2);
    let surface2 = rgb(p.surface2.0, p.surface2.1, p.surface2.2);
    let text = rgb(p.text.0, p.text.1, p.text.2);
    let subtext0 = rgb(p.subtext0.0, p.subtext0.1, p.subtext0.2);
    let overlay0 = rgb(p.overlay0.0, p.overlay0.1, p.overlay0.2);
    let blue = rgb(p.blue.0, p.blue.1, p.blue.2);
    let green = rgb(p.green.0, p.green.1, p.green.2);
    let yellow = rgb(p.yellow.0, p.yellow.1, p.yellow.2);
    let peach = rgb(p.peach.0, p.peach.1, p.peach.2);
    let red = rgb(p.red.0, p.red.1, p.red.2);

    let add_t = if is_light { 0.18 } else { 0.20 };
    let add_word_t = if is_light { 0.30 } else { 0.28 };
    let add_sel_t = if is_light { 0.22 } else { 0.24 };
    let del_t = if is_light { 0.18 } else { 0.20 };
    let del_word_t = if is_light { 0.30 } else { 0.28 };
    let del_sel_t = if is_light { 0.22 } else { 0.24 };
    let hunk_t = if is_light { 0.16 } else { 0.22 };
    let hunk_expand_t = if is_light { 0.26 } else { 0.30 };

    ThemeSpec {
        name,
        ui: UiTheme {
            bg_default: base,
            bg_sidebar: mantle,
            bg_header: base,
            bg_file_header: base,
            bg_selected: surface0,
            bg_hover: surface1,
            bg_addition_margin: blend(base, green, add_t),
            bg_deletion_margin: blend(base, red, del_t),
            bg_addition_line: blend(base, green, add_t * 0.75),
            bg_deletion_line: blend(base, red, del_t * 0.75),
            bg_addition_word: blend(base, green, add_word_t),
            bg_deletion_word: blend(base, red, del_word_t),
            bg_addition_selected: blend(base, green, add_sel_t),
            bg_deletion_selected: blend(base, red, del_sel_t),
            bg_context_selected: surface0,
            bg_hunk_header: blend(base, blue, hunk_t),
            bg_hunk_expand: blend(base, blue, hunk_expand_t),
            fg_default: text,
            fg_muted: subtext0,
            fg_addition: green,
            fg_deletion: red,
            fg_line_num: overlay0,
            fg_line_num_highlight: text,
            fg_hunk: blue,
            fg_path: blue,
            fg_directory: blue,
            fg_border: if is_light { surface2 } else { overlay0 },
            fg_stats_bar: subtext0,
            fg_warning: yellow,
            fg_cursor: peach,
            border_top_left: "╭",
            border_top_right: "╮",
            border_bottom_left: "╰",
            border_bottom_right: "╯",
            border_horizontal: "─",
            border_vertical: "│",
        },
        syntax: SyntaxPalette {
            fg: syn(p.text.0, p.text.1, p.text.2),
            background: syn(p.base.0, p.base.1, p.base.2),
            comment: syn(p.overlay0.0, p.overlay0.1, p.overlay0.2),
            keyword: syn(p.mauve.0, p.mauve.1, p.mauve.2),
            string: syn(p.green.0, p.green.1, p.green.2),
            constant: syn(p.peach.0, p.peach.1, p.peach.2),
            entity: syn(p.blue.0, p.blue.1, p.blue.2),
            tag: syn(p.red.0, p.red.1, p.red.2),
            variable: syn(p.lavender.0, p.lavender.1, p.lavender.2),
        },
    }
}

fn catppuccin_mocha() -> ThemeSpec {
    catppuccin_theme(
        "catppuccin-mocha",
        CatppuccinPalette {
            base: (30, 30, 46),
            mantle: (24, 24, 37),
            surface0: (49, 50, 68),
            surface1: (69, 71, 90),
            surface2: (88, 91, 112),
            text: (205, 214, 244),
            subtext0: (166, 173, 200),
            overlay0: (108, 112, 134),
            blue: (137, 180, 250),
            lavender: (180, 190, 254),
            mauve: (203, 166, 247),
            green: (166, 227, 161),
            yellow: (249, 226, 175),
            peach: (250, 179, 135),
            red: (243, 139, 168),
        },
        false,
    )
}

fn catppuccin_macchiato() -> ThemeSpec {
    catppuccin_theme(
        "catppuccin-macchiato",
        CatppuccinPalette {
            base: (36, 39, 58),
            mantle: (30, 32, 48),
            surface0: (54, 58, 79),
            surface1: (73, 77, 100),
            surface2: (91, 96, 120),
            text: (202, 211, 245),
            subtext0: (165, 173, 203),
            overlay0: (110, 115, 141),
            blue: (138, 173, 244),
            lavender: (183, 189, 248),
            mauve: (198, 160, 246),
            green: (166, 218, 149),
            yellow: (238, 212, 159),
            peach: (245, 169, 127),
            red: (237, 135, 150),
        },
        false,
    )
}

fn catppuccin_frappe() -> ThemeSpec {
    catppuccin_theme(
        "catppuccin-frappe",
        CatppuccinPalette {
            base: (48, 52, 70),
            mantle: (41, 44, 60),
            surface0: (65, 69, 89),
            surface1: (81, 87, 109),
            surface2: (98, 104, 128),
            text: (198, 208, 245),
            subtext0: (165, 173, 206),
            overlay0: (115, 121, 148),
            blue: (140, 170, 238),
            lavender: (186, 187, 241),
            mauve: (202, 158, 230),
            green: (166, 209, 137),
            yellow: (229, 200, 144),
            peach: (239, 159, 118),
            red: (231, 130, 132),
        },
        false,
    )
}

fn catppuccin_latte() -> ThemeSpec {
    catppuccin_theme(
        "catppuccin-latte",
        CatppuccinPalette {
            base: (239, 241, 245),
            mantle: (230, 233, 239),
            surface0: (204, 208, 218),
            surface1: (188, 192, 204),
            surface2: (172, 176, 190),
            text: (76, 79, 105),
            subtext0: (108, 111, 133),
            overlay0: (156, 160, 176),
            blue: (30, 102, 245),
            lavender: (114, 135, 253),
            mauve: (136, 57, 239),
            green: (64, 160, 43),
            yellow: (223, 142, 29),
            peach: (254, 100, 11),
            red: (210, 15, 57),
        },
        true,
    )
}

// ─── Syntect theme builder ───────────────────────────────────────────────────

fn build_syntect_theme(palette: &SyntaxPalette, name: &str) -> Theme {
    Theme {
        name: Some(name.to_string()),
        author: Some("panko".into()),
        settings: ThemeSettings {
            foreground: Some(palette.fg),
            background: Some(palette.background),
            ..Default::default()
        },
        scopes: vec![
            theme_item("comment", palette.comment, false, true),
            theme_item("punctuation.definition.comment", palette.comment, false, true),
            theme_item("keyword", palette.keyword, false, false),
            theme_item("keyword.control", palette.keyword, false, false),
            theme_item("keyword.operator", palette.keyword, false, false),
            theme_item("storage", palette.keyword, false, false),
            theme_item("storage.type", palette.keyword, false, false),
            theme_item("storage.modifier", palette.keyword, false, false),
            theme_item("string", palette.string, false, false),
            theme_item("punctuation.definition.string", palette.string, false, false),
            theme_item("constant", palette.constant, false, false),
            theme_item("constant.numeric", palette.constant, false, false),
            theme_item("constant.language", palette.constant, false, false),
            theme_item("variable.other.constant", palette.constant, false, false),
            theme_item("entity.name.function", palette.entity, false, false),
            theme_item("entity.name.method", palette.entity, false, false),
            theme_item("support.function", palette.entity, false, false),
            theme_item("meta.function-call", palette.entity, false, false),
            theme_item("entity.name.type", palette.entity, false, false),
            theme_item("entity.name.class", palette.entity, false, false),
            theme_item("support.type", palette.constant, false, false),
            theme_item("support.class", palette.constant, false, false),
            theme_item("entity.name.tag", palette.tag, false, false),
            theme_item("variable", palette.variable, false, false),
            theme_item("variable.parameter", palette.fg, false, false),
            theme_item("variable.other", palette.fg, false, false),
            theme_item("meta.property-name", palette.constant, false, false),
            theme_item("support.variable.property", palette.constant, false, false),
            theme_item("punctuation", palette.fg, false, false),
            theme_item("keyword.operator", palette.fg, false, false),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_available_themes() {
        let names = available_themes();
        assert!(names.contains(&"github-dark".to_string()));
        assert!(names.contains(&"catppuccin-mocha".to_string()));
    }
}

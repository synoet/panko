//! Terminal port (trait).
//! Defines the interface for terminal operations without coupling to crossterm/ratatui.

#![allow(dead_code)]

use anyhow::Result;
use ratatui::Frame;
use std::time::Duration;

/// Events that can occur in the terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalEvent {
    Key(KeyEvent),
    Resize(u16, u16),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyEvent {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCode {
    Char(char),
    Enter,
    Esc,
    Up,
    Down,
    Left,
    Right,
    Tab,
    BackTab,
    Backspace,
    Home,
    End,
    PageUp,
    PageDown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeyModifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

impl KeyModifiers {
    pub const NONE: Self = Self {
        ctrl: false,
        alt: false,
        shift: false,
    };

    pub const CTRL: Self = Self {
        ctrl: true,
        alt: false,
        shift: false,
    };
}

/// Port for terminal operations.
pub trait Terminal {
    /// Draw a frame to the terminal.
    fn draw<F>(&mut self, f: F) -> Result<()>
    where
        F: FnOnce(&mut Frame);

    /// Poll for an event with timeout.
    /// Returns None if no event within timeout.
    fn poll_event(&self, timeout: Duration) -> Result<Option<TerminalEvent>>;

    /// Get terminal size (width, height).
    fn size(&self) -> Result<(u16, u16)>;
}

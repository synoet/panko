//! Crossterm implementation of the Terminal port.

use crate::ports::{KeyCode, KeyEvent, KeyModifiers, Terminal, TerminalEvent};
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode as CtKeyCode, KeyModifiers as CtKeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Frame, Terminal as RatatuiTerminal};
use std::io::{self, Stdout};
use std::time::Duration;

pub struct CrosstermTerminal {
    terminal: RatatuiTerminal<CrosstermBackend<Stdout>>,
}

impl CrosstermTerminal {
    pub fn new() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = RatatuiTerminal::new(backend)?;
        Ok(Self { terminal })
    }
}

impl Drop for CrosstermTerminal {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
    }
}

impl Terminal for CrosstermTerminal {
    fn draw<F>(&mut self, f: F) -> Result<()>
    where
        F: FnOnce(&mut Frame),
    {
        self.terminal.draw(f)?;
        Ok(())
    }

    fn poll_event(&self, timeout: Duration) -> Result<Option<TerminalEvent>> {
        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) => {
                    let code = convert_key_code(key.code);
                    if let Some(code) = code {
                        return Ok(Some(TerminalEvent::Key(KeyEvent {
                            code,
                            modifiers: convert_modifiers(key.modifiers),
                        })));
                    }
                }
                Event::Resize(w, h) => {
                    return Ok(Some(TerminalEvent::Resize(w, h)));
                }
                _ => {}
            }
        }
        Ok(None)
    }

    fn size(&self) -> Result<(u16, u16)> {
        let size = self.terminal.size()?;
        Ok((size.width, size.height))
    }
}

fn convert_key_code(code: CtKeyCode) -> Option<KeyCode> {
    match code {
        CtKeyCode::Char(c) => Some(KeyCode::Char(c)),
        CtKeyCode::Enter => Some(KeyCode::Enter),
        CtKeyCode::Esc => Some(KeyCode::Esc),
        CtKeyCode::Up => Some(KeyCode::Up),
        CtKeyCode::Down => Some(KeyCode::Down),
        CtKeyCode::Left => Some(KeyCode::Left),
        CtKeyCode::Right => Some(KeyCode::Right),
        CtKeyCode::Tab => Some(KeyCode::Tab),
        CtKeyCode::BackTab => Some(KeyCode::BackTab),
        CtKeyCode::Backspace => Some(KeyCode::Backspace),
        CtKeyCode::Home => Some(KeyCode::Home),
        CtKeyCode::End => Some(KeyCode::End),
        CtKeyCode::PageUp => Some(KeyCode::PageUp),
        CtKeyCode::PageDown => Some(KeyCode::PageDown),
        _ => None,
    }
}

fn convert_modifiers(mods: CtKeyModifiers) -> KeyModifiers {
    KeyModifiers {
        ctrl: mods.contains(CtKeyModifiers::CONTROL),
        alt: mods.contains(CtKeyModifiers::ALT),
        shift: mods.contains(CtKeyModifiers::SHIFT),
    }
}

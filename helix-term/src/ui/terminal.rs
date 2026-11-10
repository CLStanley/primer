//! Compositor component for a live terminal session.
//!
//! This component is intentionally not wired into commands or editor tabs yet.
//! It establishes the UI boundary: PTY output is drained and painted without
//! converting terminal contents into an editable text document.

use crate::compositor::{Component, Context, Event, EventResult};
use crate::terminal::input::encode_key;
use crate::terminal::TerminalInputMode;
use helix_view::keyboard::{KeyCode, KeyModifiers};
use crate::terminal::{render::render_terminal, runtime::RunningTerminal};
use helix_view::graphics::Rect;
use tui::buffer::Buffer;

/// A terminal surface owned by the compositor.
///
/// The future terminal-buffer integration can reuse this surface and its
/// runtime while associating it with a stable terminal session identifier.
pub struct TerminalView {
    terminal: RunningTerminal,
    last_size: Option<(u16, u16)>,
    error: Option<String>,
    input_mode: TerminalInputMode,
    escape_pending: bool,
}

impl TerminalView {
    pub fn new(terminal: RunningTerminal) -> Self {
        Self {
            terminal,
            last_size: None,
            error: None,
            input_mode: TerminalInputMode::Terminal,
            escape_pending: false,
        }
    }

    pub fn terminal(&self) -> &RunningTerminal {
        &self.terminal
    }

    pub fn terminal_mut(&mut self) -> &mut RunningTerminal {
        &mut self.terminal
    }

    pub fn input_mode(&self) -> TerminalInputMode {
        self.input_mode
    }

    pub fn set_input_mode(&mut self, mode: TerminalInputMode) {
        self.input_mode = mode;
        self.escape_pending = false;
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

impl Component for TerminalView {
    fn handle_event(&mut self, event: &Event, _ctx: &mut Context) -> EventResult {
        if self.input_mode == TerminalInputMode::Normal {
            return EventResult::Ignored(None);
        }
        match event {
            Event::Key(key) => {
                let prefix = key.code == KeyCode::Char('\\')
                    && key.modifiers.contains(KeyModifiers::CONTROL);
                if self.escape_pending {
                    self.escape_pending = false;
                    if key.code == KeyCode::Char('n')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        self.input_mode = TerminalInputMode::Normal;
                        return EventResult::Consumed(None);
                    }
                    if let Err(err) = self.terminal.write_input(&[28]) {
                        self.error = Some(err.to_string());
                    }
                } else if prefix {
                    self.escape_pending = true;
                    return EventResult::Consumed(None);
                }
                if let Some(bytes) = encode_key(*key) {
                    if let Err(err) = self.terminal.write_input(&bytes) {
                        self.error = Some(err.to_string());
                    }
                }
                EventResult::Consumed(None)
            }
            Event::Paste(text) => {
                if self.escape_pending {
                    self.escape_pending = false;
                    let _ = self.terminal.write_input(&[28]);
                }
                if let Err(err) = self.terminal.write_input(text.as_bytes()) {
                    self.error = Some(err.to_string());
                }
                EventResult::Consumed(None)
            }
            _ => EventResult::Ignored(None),
        }
    }
    fn render(&mut self, area: Rect, surface: &mut Buffer, cx: &mut Context) {
        // A zero-sized pane must not trigger an invalid PTY resize.
        if area.width > 0 && area.height > 0 {
            let size = (area.width, area.height);
            if self.last_size != Some(size) {
                match self.terminal.resize(size.0, size.1) {
                    Ok(()) => self.last_size = Some(size),
                    Err(err) => self.error = Some(err.to_string()),
                }
            }
        }

        if let Err(err) = self.terminal.drain_output() {
            self.error = Some(err.to_string());
        }

        render_terminal(
            self.terminal.screen(),
            area,
            surface,
            cx.editor.theme.get("ui.text"),
        );
    }
}

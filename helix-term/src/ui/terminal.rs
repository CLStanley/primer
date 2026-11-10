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
use helix_view::graphics::{Rect, CursorKind};
use helix_core::Position;
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
            if let Event::Key(key) = event {
                if key.modifiers.is_empty() {
                    match key.code {
                        KeyCode::Char('i') => {
                            self.set_input_mode(TerminalInputMode::Terminal);
                            return EventResult::Consumed(None);
                        }
                        KeyCode::Char('q') => {
                            let _ = self.terminal.kill();
                            return EventResult::Consumed(Some(Box::new(|compositor, _| {
                                compositor.remove_type::<TerminalView>();
                            })));
                        }
                        _ => {}
                    }
                }
            }
            // Do not accidentally edit the underlying document while a
            // terminal compositor layer is focused.
            return EventResult::Consumed(None);
        }
        match event {
            Event::Key(key) => {
                if key.code == KeyCode::Esc && key.modifiers.is_empty() {
                    self.set_input_mode(TerminalInputMode::Normal);
                    return EventResult::Consumed(None);
                }
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
    fn cursor(&self, area: Rect, _editor: &helix_view::Editor) -> (Option<Position>, CursorKind) {
        if self.input_mode != TerminalInputMode::Terminal || area.width == 0 || area.height <= 1 {
            return (None, CursorKind::Hidden);
        }
        let screen = self.terminal.screen().screen();
        if screen.hide_cursor() {
            return (None, CursorKind::Hidden);
        }
        let (row, col) = screen.cursor_position();
        if col >= area.width || row >= area.height - 1 {
            return (None, CursorKind::Hidden);
        }
        (Some(Position::new(area.y as usize + row as usize, area.x as usize + col as usize)), CursorKind::Bar)
    }

    fn render(&mut self, area: Rect, surface: &mut Buffer, cx: &mut Context) {
        let terminal_area = Rect::new(area.x, area.y, area.width, area.height.saturating_sub(1));
        // Reserve the bottom row for a terminal mode/status line.
        // A zero-sized pane must not trigger an invalid PTY resize.
        if terminal_area.width > 0 && terminal_area.height > 0 {
            let size = (terminal_area.width, terminal_area.height);
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
            terminal_area,
            surface,
            cx.editor.theme.get("ui.text"),
        );
        if area.height > 0 {
            let label = match self.input_mode {
                TerminalInputMode::Terminal => " TERMINAL  Esc: normal ",
                TerminalInputMode::Normal => " NORMAL  i: terminal  q: close ",
            };
            let status = cx.editor.theme.get("ui.statusline");
            let status_area = Rect::new(area.x, area.y + area.height - 1, area.width, 1);
            surface.set_style(status_area, status);
            surface.set_stringn(status_area.x, status_area.y, &" ".repeat(area.width as usize), area.width as usize, status);
            surface.set_stringn(status_area.x, status_area.y, label, area.width as usize, status);
        }
    }
}

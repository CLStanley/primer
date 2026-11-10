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
use helix_view::graphics::{Rect, CursorKind, Color, Modifier};
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
    close_sequence: u8,
    confirm_close: bool,
}

impl TerminalView {
    pub fn new(terminal: RunningTerminal) -> Self {
        Self {
            terminal,
            last_size: None,
            error: None,
            input_mode: TerminalInputMode::Terminal,
            escape_pending: false,
            close_sequence: 0,
            confirm_close: false,
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
                    if self.confirm_close {
                        self.confirm_close = false;
                        self.close_sequence = 0;
                        if key.code == KeyCode::Char('y') {
                            let _ = self.terminal.kill();
                            return EventResult::Consumed(Some(Box::new(|compositor, _| {
                                compositor.remove_type::<TerminalView>();
                            })));
                        }
                        return EventResult::Consumed(None);
                    }
                    match (self.close_sequence, key.code) {
                        (0, KeyCode::Char(' ')) => {
                            self.close_sequence = 1;
                            return EventResult::Consumed(None);
                        }
                        (1, KeyCode::Char('b')) => {
                            self.close_sequence = 2;
                            return EventResult::Consumed(None);
                        }
                        (2, KeyCode::Char('q')) => {
                            self.close_sequence = 0;
                            self.confirm_close = true;
                            return EventResult::Consumed(None);
                        }
                        (_, KeyCode::Char('i')) => {
                            self.close_sequence = 0;
                            self.set_input_mode(TerminalInputMode::Terminal);
                            return EventResult::Consumed(None);
                        }
                        _ => self.close_sequence = 0,
                    }
                } else {
                    self.close_sequence = 0;
                }
            }
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
        let pane = Self::terminal_area(area);
        if self.input_mode != TerminalInputMode::Terminal || pane.width == 0 || pane.height == 0 {
            return (None, CursorKind::Hidden);
        }
        let screen = self.terminal.screen().screen();
        if screen.hide_cursor() {
            return (None, CursorKind::Hidden);
        }
        let (row, col) = screen.cursor_position();
        if col >= pane.width || row >= pane.height {
            return (None, CursorKind::Hidden);
        }
        (Some(Position::new((pane.y + row) as usize, (pane.x + col) as usize)), CursorKind::Bar)
    }

    fn render(&mut self, area: Rect, surface: &mut Buffer, cx: &mut Context) {
        let pane = Self::terminal_area(area);
        if pane.width > 0 && pane.height > 0 {
            let size = (pane.width, pane.height);
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

        render_terminal(self.terminal.screen(), pane, surface, cx.editor.theme.get("ui.text").fg(Color::LightGray));

        if area.height >= 3 {
            let tab = Rect::new(area.x, area.y, area.width, 1);
            let tab_style = cx.editor.theme.get("ui.bufferline.active").add_modifier(Modifier::BOLD);
            surface.set_style(tab, cx.editor.theme.get("ui.background"));
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "sh".into());
            let name = std::path::Path::new(&shell)
                .file_name().and_then(|name| name.to_str()).unwrap_or("terminal");
            let title = format!("  {name}  ");
            surface.set_stringn(tab.x, tab.y, &title, tab.width as usize, tab_style);

            let status = Rect::new(area.x, area.y + area.height - 2, area.width, 1);
            let status_style = cx.editor.theme.get("ui.statusline");
            surface.set_style(status, status_style);
            surface.set_stringn(status.x, status.y, &" ".repeat(status.width as usize), status.width as usize, status_style);
            let (columns, rows) = self.terminal.screen().size();
            let mode = if self.input_mode == TerminalInputMode::Terminal { "TERMINAL" } else { "NORMAL" };
            let detail = format!("  {mode}  │  {name}  │  {columns}×{rows}");
            surface.set_stringn(status.x, status.y, &detail, status.width as usize, status_style);

            let command = Rect::new(area.x, area.y + area.height - 1, area.width, 1);
            let command_style = cx.editor.theme.get("ui.text");
            surface.set_style(command, cx.editor.theme.get("ui.background"));
            surface.set_stringn(command.x, command.y, &" ".repeat(command.width as usize), command.width as usize, command_style);
            let message = if self.confirm_close {
                "Kill terminal and running processes? [y/N]"
            } else if self.input_mode == TerminalInputMode::Terminal {
                "-- TERMINAL --"
            } else {
                ""
            };
            surface.set_stringn(command.x, command.y, message, command.width as usize, command_style);
        }
    }
}

impl TerminalView {
    /// Keep the terminal grid inside the editor chrome: tab, statusline, commandline.
    fn terminal_area(area: Rect) -> Rect {
        if area.height < 3 {
            return Rect::new(area.x, area.y, area.width, 0);
        }
        Rect::new(area.x, area.y + 1, area.width, area.height - 3)
    }
}

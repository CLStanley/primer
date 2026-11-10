//! Compositor component for a live terminal session.
//!
//! This component is intentionally not wired into commands or editor tabs yet.
//! It establishes the UI boundary: PTY output is drained and painted without
//! converting terminal contents into an editable text document.

use crate::compositor::{Component, Context, Event, EventResult};
use crate::terminal::input::encode_key;
use crate::keymap::KeyTrie;
use helix_view::document::Mode;
use crate::terminal::TerminalInputMode;
use helix_view::keyboard::{KeyCode, KeyModifiers};
use crate::terminal::{render::render_terminal, runtime::RunningTerminal};
use helix_view::graphics::{Rect, CursorKind, Color, Modifier, UnderlineStyle};
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
                            return EventResult::Consumed(Some(Box::new(|compositor, cx| {
                                compositor.remove_type::<TerminalView>();
                                // Restore the home screen when there is no real file to show.
                                // Do not discard existing named editing buffers.
                                if !cx.editor.documents().any(|doc| doc.path().is_some())
                                    && compositor.find::<crate::ui::dashboard::Dashboard>().is_none()
                                {
                                    compositor.push(Box::new(crate::ui::dashboard::Dashboard::new()));
                                }
                            })));
                        }
                        return EventResult::Consumed(None);
                    }
                    // Let Primer's real editor command-line keymap open the
                    // prompt. The prompt becomes the top compositor layer, so
                    // subsequent keystrokes go to it rather than the PTY.
                    if self.close_sequence == 0 && key.code == KeyCode::Char(':') {
                        return EventResult::Ignored(None);
                    }
                    match (self.close_sequence, key.code) {
                        (0, KeyCode::Char(' ')) => {
                            self.close_sequence = 1;
                            // Read the real normal-mode keymap for its descriptions,
                            // but do not execute editor commands against the document
                            // behind this terminal.
                            return EventResult::Consumed(Some(Box::new(|compositor, cx| {
                                if let Some(view) = compositor.find::<crate::ui::EditorView>() {
                                    let key = crate::key!(' ');
                                    let keymaps = view.keymaps.map();
                                    if let Some(KeyTrie::Node(node)) = keymaps[&Mode::Normal].search(&[key]) {
                                        cx.editor.autoinfo = Some(node.infobox());
                                    }
                                }
                            })));
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
            let background = cx.editor.theme.try_get("ui.bufferline.background")
                .unwrap_or_else(|| cx.editor.theme.get("ui.background"));
            let active = cx.editor.theme.try_get("ui.bufferline.active")
                .unwrap_or_else(|| cx.editor.theme.get("ui.statusline.active"))
                .bg(cx.editor.theme.get("ui.statusline.normal").bg.unwrap_or(Color::Rgb(183, 156, 222)))
                .fg(cx.editor.theme.get("ui.statusline.normal").fg.unwrap_or(Color::Rgb(29, 22, 43)))
                .add_modifier(Modifier::BOLD)
                .underline_style(UnderlineStyle::Reset);
            surface.set_style(tab, background);
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "sh".into());
            let name = std::path::Path::new(&shell)
                .file_name().and_then(|name| name.to_str()).unwrap_or("terminal");
            let title = format!("  {name}  ");
            let mut x = tab.x;
            if cx.editor.config().bufferline_powerline {
                if let Some(bg) = active.bg {
                    x = surface.set_stringn(x, tab.y, "", tab.width as usize, background.fg(bg)).0;
                }
            }
            x = surface.set_stringn(x, tab.y, &title, tab.right().saturating_sub(x) as usize, active).0;
            if cx.editor.config().bufferline_powerline {
                if let Some(bg) = active.bg {
                    surface.set_stringn(x, tab.y, "", tab.right().saturating_sub(x) as usize, background.fg(bg));
                }
            }

            let status = Rect::new(area.x, area.y + area.height - 2, area.width, 1);
            let mut base = cx.editor.theme.get("ui.statusline");
            base.bg = Some(Color::Reset);
            surface.set_style(status, base);
            surface.set_stringn(status.x, status.y, &" ".repeat(status.width as usize), status.width as usize, base);

            let terminal_mode = self.input_mode == TerminalInputMode::Terminal;
            let mode_name = if terminal_mode { "TERMINAL" } else { "NORMAL" };
            let mode_style = cx.editor.theme.try_get(if terminal_mode {
                "ui.statusline.insert"
            } else {
                "ui.statusline.normal"
            }).unwrap_or_else(|| cx.editor.theme.get("ui.statusline.active"))
              .add_modifier(Modifier::BOLD);
            let mode_text = format!("   {mode_name}  ");
            let mode_bg = mode_style.bg.unwrap_or(Color::Rgb(183, 156, 222));
            let mut mode_x = status.x;
            mode_x = surface.set_stringn(mode_x, status.y, "", status.width as usize, base.fg(mode_bg)).0;
            mode_x = surface.set_stringn(mode_x, status.y, &mode_text, status.right().saturating_sub(mode_x) as usize, mode_style).0;
            surface.set_stringn(mode_x, status.y, "", status.right().saturating_sub(mode_x) as usize, base.fg(mode_bg));

            let (columns, rows) = self.terminal.screen().size();
            let dimensions = format!(" {columns}×{rows} ");
            let shell_text = format!("  {name} ");
            let shell_style = cx.editor.theme.get("ui.statusline.normal").add_modifier(Modifier::BOLD);
            let shell_width = shell_text.chars().count() as u16 + 2;
            let dim_width = dimensions.chars().count() as u16;
            if status.width > shell_width + dim_width + mode_text.chars().count() as u16 {
                let shell_x = status.right() - shell_width;
                let dim_x = shell_x - dim_width;
                surface.set_stringn(dim_x, status.y, &dimensions, dim_width as usize, base);
                let shell_bg = shell_style.bg.unwrap_or(Color::Rgb(183, 156, 222));
                surface.set_stringn(shell_x, status.y, "", 1, base.fg(shell_bg));
                surface.set_stringn(shell_x + 1, status.y, &shell_text, shell_text.chars().count(), shell_style);
                surface.set_stringn(status.right() - 1, status.y, "", 1, base.fg(shell_bg));
            }

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

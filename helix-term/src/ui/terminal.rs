//! Compositor component for a live terminal session.
//!
//! Each PTY is associated with an editor document, so buffer navigation and
//! the shared bufferline can select it without converting PTY output into text.

use crate::compositor::{Component, Context, Event, EventResult};
use crate::keymap::KeyTrie;
use crate::terminal::input::encode_key;
use crate::terminal::TerminalInputMode;
use crate::terminal::{render::render_terminal, runtime::RunningTerminal};
use helix_core::Position;
use helix_view::document::Mode;
use helix_view::graphics::{Color, CursorKind, Rect};
use helix_view::input::KeyEvent;
use helix_view::keyboard::{KeyCode, KeyModifiers};
use tui::buffer::Buffer;

/// PTY-backed terminal surface associated with an editor document.
pub struct TerminalView {
    terminal: RunningTerminal,
    document_id: helix_view::DocumentId,
    last_size: Option<(u16, u16)>,
    error: Option<String>,
    input_mode: TerminalInputMode,
    escape_pending: bool,
    close_sequence: u8,
    pending_keys: Vec<KeyEvent>,
    confirm_close: bool,
}

impl TerminalView {
    pub fn new(terminal: RunningTerminal, document_id: helix_view::DocumentId) -> Self {
        Self {
            terminal,
            document_id,
            last_size: None,
            error: None,
            input_mode: TerminalInputMode::Terminal,
            escape_pending: false,
            close_sequence: 0,
            pending_keys: Vec::new(),
            confirm_close: false,
        }
    }

    /// Advance through editor documents, including terminal-backed buffers.
    fn switch_buffer(
        editor: &mut helix_view::Editor,
        forward: bool,
    ) {
        let current = editor.tree.get(editor.tree.focus).doc;
        let ids: Vec<_> = editor.documents.keys().copied().collect();
        if ids.len() < 2 {
            return;
        }
        if let Some(index) = ids.iter().position(|id| *id == current) {
            let next = if forward {
                (index + 1) % ids.len()
            } else {
                (index + ids.len() - 1) % ids.len()
            };
            editor.switch(ids[next], helix_view::editor::Action::Replace);
        }
    }

    pub fn document_id(&self) -> helix_view::DocumentId {
        self.document_id
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
        self.pending_keys.clear();
        self.close_sequence = 0;
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

impl Component for TerminalView {
    fn handle_event(&mut self, event: &Event, ctx: &mut Context) -> EventResult {
        if self.input_mode == TerminalInputMode::Normal {
            if let Event::Key(key) = event {
                // Terminal Normal Mode shares the editor's buffer navigation.
                // Terminals may report shifted letters with a SHIFT modifier.
                if self.close_sequence == 0
                    && (key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT)
                    && matches!(key.code, KeyCode::Char('H') | KeyCode::Char('L'))
                {
                    let forward = key.code == KeyCode::Char('L');
                    return EventResult::Consumed(Some(Box::new(move |compositor, cx| {
                        TerminalView::switch_buffer(cx.editor, forward);
                        compositor.sync_terminal_document(cx.editor);
                    })));
                }
                if key.modifiers.is_empty() {
                    if self.confirm_close {
                        self.confirm_close = false;
                        self.close_sequence = 0;
                        if key.code == KeyCode::Char('y') {
                            let _ = self.terminal.kill();
                            return EventResult::Consumed(Some(Box::new(|compositor, cx| {
                                let id = cx.editor.tree.get(cx.editor.tree.focus).doc;
                                if let Err(_err) = cx.editor.close_document(id, true) {
                                    cx.editor.terminal_documents.remove(&id);
                                }
                                compositor.remove_terminal_document(id);
                                // Restore the home screen when there is no real file to show.
                                // Do not discard existing named editing buffers.
                                if !cx.editor.documents().any(|doc| doc.path().is_some())
                                    && cx.editor.terminal_documents.is_empty()
                                    && compositor
                                        .find::<crate::ui::dashboard::Dashboard>()
                                        .is_none()
                                {
                                    compositor
                                        .push(Box::new(crate::ui::dashboard::Dashboard::new()));
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
                    // Escape cancels an unfinished terminal key family, including
                    // its shared Which-Key popup, without closing the terminal.
                    if key.code == KeyCode::Esc {
                        self.close_sequence = 0;
                        self.pending_keys.clear();
                        ctx.editor.autoinfo = None;
                        return EventResult::Consumed(None);
                    }
                    match (self.close_sequence, key.code) {
                        (0, KeyCode::Char(' ')) => {
                            self.close_sequence = 1;
                            self.pending_keys.clear();
                            self.pending_keys.push(*key);
                            let keys = self.pending_keys.clone();
                            return EventResult::Consumed(Some(Box::new(move |compositor, cx| {
                                if let Some(view) = compositor.find::<crate::ui::EditorView>() {
                                    let keymaps = view.keymaps.map();
                                    if let Some(KeyTrie::Node(node)) =
                                        keymaps[&Mode::Normal].search(&keys)
                                    {
                                        cx.editor.autoinfo = Some(node.infobox());
                                    }
                                }
                            })));
                        }
                        (1, KeyCode::Char('b')) | (2, KeyCode::Char('q')) => {
                            let closing = self.close_sequence == 2;
                            self.pending_keys.push(*key);
                            self.close_sequence = if closing { 0 } else { 2 };
                            if closing {
                                self.pending_keys.clear();
                                ctx.editor.autoinfo = None;
                                self.confirm_close = true;
                                return EventResult::Consumed(None);
                            }
                            let keys = self.pending_keys.clone();
                            return EventResult::Consumed(Some(Box::new(move |compositor, cx| {
                                if let Some(view) = compositor.find::<crate::ui::EditorView>() {
                                    let keymaps = view.keymaps.map();
                                    if let Some(KeyTrie::Node(node)) =
                                        keymaps[&Mode::Normal].search(&keys)
                                    {
                                        cx.editor.autoinfo = Some(node.infobox());
                                    } else {
                                        cx.editor.autoinfo = None;
                                    }
                                }
                            })));
                        }
                        (_, KeyCode::Char('i')) => {
                            self.close_sequence = 0;
                            self.set_input_mode(TerminalInputMode::Terminal);
                            return EventResult::Consumed(None);
                        }
                        _ if !self.pending_keys.is_empty() => {
                            self.pending_keys.push(*key);
                            let keys = self.pending_keys.clone();
                            // Keep the sequence alive while traversing a nested
                            // family. Only a completed or invalid binding resets it.
                            return EventResult::Consumed(Some(Box::new(move |compositor, cx| {
                                let result =
                                    compositor.find::<crate::ui::EditorView>().and_then(|view| {
                                        let keymaps = view.keymaps.map();
                                        keymaps[&Mode::Normal].search(&keys).cloned()
                                    });
                                match result {
                                    Some(KeyTrie::Node(node)) => {
                                        cx.editor.autoinfo = Some(node.infobox());
                                        // Preserve the pending keys for the next level.
                                    }
                                    Some(KeyTrie::MappableCommand(command)) => {
                                        if let Some(terminal) = compositor.find_terminal_document(cx.editor.tree.get(cx.editor.tree.focus).doc) {
                                            terminal.pending_keys.clear();
                                            terminal.close_sequence = 0;
                                        }
                                        cx.editor.autoinfo = None;
                                        // Switching away from the terminal should
                                        // reveal the editor's currently selected buffer.
                                        // The PTY remains parked and can be resumed
                                        // with :terminal.
                                        if matches!(
                                            command.name(),
                                            "primer_buffer_next"
                                                | "primer_buffer_previous"
                                                | "buffer_next"
                                                | "buffer_previous"
                                        ) {
                                            let forward = matches!(
                                                command.name(),
                                                "primer_buffer_next" | "buffer_next"
                                            );
                                            TerminalView::switch_buffer(cx.editor, forward);
                                            compositor.sync_terminal_document(cx.editor);
                                            return;
                                        }
                                        // Editor-wide commands, including creating a
                                        // new PTY, work from terminal Normal Mode.
                                        if matches!(
                                            command.name(),
                                            "file_picker"
                                                | "file_picker_in_current_directory"
                                                | "file_explorer"
                                                | "file_explorer_in_current_buffer_directory"
                                                | "global_search"
                                                | "command_palette"
                                                | "buffer_picker"
                                                | "primer_agenda"
                                                | "primer_terminal"
                                        ) {
                                            let mut command_cx = crate::commands::Context {
                                                editor: cx.editor,
                                                count: None,
                                                register: None,
                                                callback: Vec::new(),
                                                on_next_key_callback: None,
                                                jobs: cx.jobs,
                                            };
                                            command.execute(&mut command_cx);
                                            let callbacks =
                                                std::mem::take(&mut command_cx.callback);
                                            drop(command_cx);
                                            for callback in callbacks {
                                                callback(compositor, cx);
                                            }
                                            // The selected editor document controls which
                                            // surface is visible after picker callbacks.
                                            compositor.sync_terminal_document(cx.editor);
                                        } else {
                                            cx.editor.set_error(format!(
                                                "Switch to an editable buffer before using {}",
                                                command.name()
                                            ));
                                        }
                                    }
                                    _ => {
                                        if let Some(terminal) = compositor.find_terminal_document(cx.editor.tree.get(cx.editor.tree.focus).doc) {
                                            terminal.pending_keys.clear();
                                            terminal.close_sequence = 0;
                                        }
                                        cx.editor.autoinfo = None;
                                    }
                                }
                            })));
                        }
                        _ => {
                            self.close_sequence = 0;
                            ctx.editor.autoinfo = None;
                        }
                    }
                } else {
                    self.close_sequence = 0;
                    ctx.editor.autoinfo = None;
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
        (
            Some(Position::new(
                (pane.y + row) as usize,
                (pane.x + col) as usize,
            )),
            CursorKind::Bar,
        )
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

        render_terminal(
            self.terminal.screen(),
            pane,
            surface,
            cx.editor.theme.get("ui.text").fg(Color::LightGray),
        );

        if area.height >= 3 {
            // The terminal uses the same document bufferline as text buffers.
            crate::ui::EditorView::render_bufferline(
                cx.editor,
                Rect::new(area.x, area.y, area.width, 1),
                surface,
            );
            let status = Rect::new(area.x, area.y + area.height - 2, area.width, 1);
            let (columns, rows) = self.terminal.screen().size();
            let name = cx.editor.terminal_names.get(&self.document_id)
                .map(String::as_str)
                .unwrap_or("terminal");
            if let Some(view) = cx.editor.tree.try_get(cx.editor.tree.focus) {
                if let Some(doc) = cx.editor.documents.get(&self.document_id) {
                    let spinners = crate::ui::ProgressSpinners::default();
                    let mut context = crate::ui::statusline::RenderContext::new(
                        cx.editor, doc, view, true, &spinners,
                    );
                    context.terminal = Some(crate::ui::statusline::TerminalStatus {
                        name,
                        input_mode: self.input_mode == TerminalInputMode::Terminal,
                        columns,
                        rows,
                    });
                    crate::ui::statusline::render(&mut context, status, surface);
                }
            }

            let command = Rect::new(area.x, area.y + area.height - 1, area.width, 1);
            let command_style = cx.editor.theme.get("ui.text");
            surface.set_style(command, cx.editor.theme.get("ui.background"));
            surface.set_stringn(
                command.x,
                command.y,
                &" ".repeat(command.width as usize),
                command.width as usize,
                command_style,
            );
            let message = if self.confirm_close {
                "Kill terminal and running processes? [y/N]"
            } else if self.input_mode == TerminalInputMode::Terminal {
                "-- TERMINAL --"
            } else {
                ""
            };
            surface.set_stringn(
                command.x,
                command.y,
                message,
                command.width as usize,
                command_style,
            );
        }

        // EditorView renders autoinfo before this compositor layer, so its
        // Which-Key panel is otherwise painted over by the terminal surface.
        // Draw the shared popup last while terminal Normal Mode is active.
        if self.input_mode == TerminalInputMode::Normal && !self.pending_keys.is_empty() {
            if let Some(mut info) = cx.editor.autoinfo.take() {
                info.render(area, surface, cx);
                cx.editor.autoinfo = Some(info);
            }
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

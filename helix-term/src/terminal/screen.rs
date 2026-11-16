//! VT escape processing and screen state for PTY-backed sessions.
//!
//! This layer consumes raw PTY bytes, including partial escape sequences,
//! and maintains a screen snapshot independently of Primer text documents.
//! Rendering into the editor's surface is a separate integration step.

/// Stateful VT parser and terminal screen.
///
/// The parser handles escape sequences across successive reads, including
/// cursor movement, alternate-screen programs and terminal resize.
pub struct TerminalScreen {
    parser: vt100::Parser,
    columns: u16,
    rows: u16,
}

impl TerminalScreen {
    pub fn new(columns: u16, rows: u16, scrollback: usize) -> anyhow::Result<Self> {
        anyhow::ensure!(columns > 0 && rows > 0, "terminal dimensions must be nonzero");
        Ok(Self {
            parser: vt100::Parser::new(rows, columns, scrollback),
            columns,
            rows,
        })
    }

    /// Feed PTY output in the order received. Partial escape sequences are
    /// retained by the parser until subsequent bytes complete them.
    pub fn process(&mut self, output: &[u8]) {
        self.parser.process(output);
    }

    pub fn resize(&mut self, columns: u16, rows: u16) -> anyhow::Result<()> {
        anyhow::ensure!(columns > 0 && rows > 0, "terminal dimensions must be nonzero");
        self.parser.set_size(rows, columns);
        self.columns = columns;
        self.rows = rows;
        Ok(())
    }

    /// Select the number of history rows displayed above the live viewport.
    pub fn set_scrollback_offset(&mut self, offset: usize) {
        self.parser.set_scrollback(offset);
    }

    pub fn scrollback_offset(&self) -> usize {
        self.parser.screen().scrollback()
    }

    pub fn size(&self) -> (u16, u16) {
        (self.columns, self.rows)
    }

    /// Access the complete VT screen, including cell attributes and cursor.
    /// The renderer should use this rather than parsing plain-text output.
    pub fn screen(&self) -> &vt100::Screen {
        self.parser.screen()
    }

    /// A plain-text snapshot for diagnostics and accessibility, not rendering.
    pub fn text_snapshot(&self) -> String {
        self.parser.screen().contents()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_incremental_output() {
        let mut screen = TerminalScreen::new(40, 10, 100).unwrap();
        screen.process(b"hello");
        screen.process(b" world");
        assert!(screen.text_snapshot().contains("hello world"));
    }

    #[test]
    fn handles_split_cursor_escape() {
        let mut screen = TerminalScreen::new(40, 10, 100).unwrap();
        screen.process(b"first");
        screen.process(b"\x1b[");
        screen.process(b"2;1Hsecond");
        let text = screen.text_snapshot();
        assert!(text.contains("first"));
        assert!(text.contains("second"));
    }

    #[test]
    fn resizing_updates_dimensions() {
        let mut screen = TerminalScreen::new(80, 24, 100).unwrap();
        screen.resize(120, 40).unwrap();
        assert_eq!(screen.size(), (120, 40));
        assert!(screen.resize(0, 40).is_err());
        assert_eq!(screen.size(), (120, 40));
    }

    #[test]
    fn rejects_zero_sized_screen() {
        assert!(TerminalScreen::new(0, 24, 0).is_err());
        assert!(TerminalScreen::new(80, 0, 0).is_err());
    }
}

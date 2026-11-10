//! Renders a VT screen into Primer's existing terminal UI surface.
//!
//! This is a reusable renderer, not yet a terminal buffer or compositor layer.
//! It clips all output to the assigned pane and leaves editor focus handling
//! to the future terminal view integration.

use helix_view::graphics::{Rect, Style};
use tui::buffer::Buffer;

use super::screen::TerminalScreen;

/// Paint the terminal's visible cell grid inside `area`.
///
/// The first version intentionally uses the surrounding Primer text style.
/// VT cell colors and attributes will be mapped to Primer styles in a
/// subsequent iteration. This renderer never writes outside `area`.
pub fn render_terminal(
    terminal: &TerminalScreen,
    area: Rect,
    surface: &mut Buffer,
    style: Style,
) {
    let screen = terminal.screen();
    let (columns, rows) = terminal.size();
    let width = area.width.min(columns);
    let height = area.height.min(rows);

    // Clear the pane first: shorter lines and terminal erase sequences must
    // not leave stale content from the previous frame.
    surface.set_style(area, style);
    for row in 0..area.height {
        surface.set_stringn(area.x, area.y + row, &" ".repeat(area.width as usize), area.width as usize, style);
    }

    for row in 0..height {
        for col in 0..width {
            let Some(cell) = screen.cell(row, col) else {
                continue;
            };
            // Continuation cells of wide glyphs have empty contents.
            let contents = cell.contents();
            if contents.is_empty() {
                continue;
            }
            let remaining = (width - col) as usize;
            surface.set_stringn(area.x + col, area.y + row, &contents, remaining, style);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renderer_handles_empty_area() {
        let terminal = TerminalScreen::new(80, 24, 0).unwrap();
        let mut surface = Buffer::empty(Rect::new(0, 0, 80, 24));
        render_terminal(&terminal, Rect::new(0, 0, 0, 0), &mut surface, Style::default());
    }
}

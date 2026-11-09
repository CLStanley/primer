use helix_view::graphics::Rect;
use tui::buffer::Buffer as Surface;
use tui::text::Text;
use tui::widgets::{Paragraph, Widget};

use super::{Dashboard, DashboardAction};
use crate::compositor::Context;

// The original eight-line Neovim dashboard logo.
// Kept as separate lines to preserve the artwork's exact horizontal spacing.
const LOGO: [&str; 8] = [
    "8888888b.  8888888b.  8888888 888b     d888 8888888888 8888888b.",
    "888   Y88b 888   Y88b   888   8888b   d8888 888        888   Y88b",
    "888    888 888    888   888   88888b.d88888 888        888    888",
    "888   d88P 888   d88P   888   888Y88888P888 8888888    888   d88P",
    "8888888P\"  8888888P\"    888   888 Y888P 888 888        8888888P\"",
    "888        888 T88b     888   888  Y8P  888 888        888 T88b",
    "888        888  T88b    888   888   \"   888 888        888  T88b",
    "888        888   T88b 8888888 888       888 8888888888 888   T88b"
];
const MENU_WIDTH: u16 = 42;

impl DashboardAction {
    pub fn label(self) -> &'static str {
        match self {
            Self::FindFile => "Find file",
            Self::RecentFiles => "Open buffers",
            Self::OpenProject => "Browse workspace",
            Self::Help => "Command palette",
        }
    }
}

pub fn render_dashboard(
    dashboard: &Dashboard,
    area: Rect,
    surface: &mut Surface,
    ctx: &mut Context,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let normal = ctx.editor.theme.get("ui.text");
    let selected = ctx.editor.theme.get("ui.selection");
    let accent = ctx.editor.theme.try_get("ui.statusline.active").unwrap_or(normal);
    surface.clear_with(area, normal);

    // Keep the full ASCII artwork intact. On narrow terminals, show a
    // compact text heading instead of clipping the logo's characters.
    let logo_width = LOGO.iter().map(|line| line.len()).max().unwrap_or(0) as u16;
    let show_logo = area.width >= logo_width && area.height >= 17;
    let heading_height: u16 = if show_logo { LOGO.len() as u16 } else { 1 };
    let content_height = heading_height + 2 + dashboard.actions.len() as u16 + 2;
    let top = area.y + area.height.saturating_sub(content_height) / 2;

    // Draw only inside the available viewport; never allow a Rect to
    // extend beyond the right or bottom edges of the terminal.
    let mut draw_line = |line: &str, row: u16, style| {
        if row >= area.bottom() {
            return;
        }
        let width = (line.len() as u16).min(area.width);
        if width == 0 {
            return;
        }
        let x = area.x + area.width.saturating_sub(width) / 2;
        let text = Text::from(line);
        Paragraph::new(&text)
            .style(style)
            .render(Rect::new(x, row, width, 1), surface);
    };

    if show_logo {
        for (index, line) in LOGO.iter().enumerate() {
            draw_line(line, top + index as u16, accent);
        }
    } else {
        draw_line("PRIMER", top, accent);
    }

    let menu_top = top + heading_height + 2;
    for (index, action) in dashboard.actions.iter().enumerate() {
        let marker = if index == dashboard.selected { ">" } else { " " };
        let label = format!("{marker}  {}", action.label());
        let style = if index == dashboard.selected { selected } else { normal };
        draw_line(&label, menu_top + index as u16, style);
    }

    if area.width >= MENU_WIDTH {
        draw_line(
            "j/k: navigate   Enter: open   Esc: edit",
            menu_top + dashboard.actions.len() as u16 + 1,
            normal,
        );
    }
}

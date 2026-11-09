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
const WIDE_LAYOUT_WIDTH: u16 = 76;
const COLUMN_GAP: u16 = 8;
const COLUMN_WIDTH: u16 = 24;

impl DashboardAction {
    pub fn label(self) -> &'static str {
        match self {
            Self::FindFile => "Browse files",
            Self::RecentFiles => "Open buffers",
            Self::OpenProject => "Browse workspace",
            Self::Help => "Keybind help",
        }
    }

    pub fn shortcut(self) -> char {
        match self {
            Self::FindFile => 'f',
            Self::RecentFiles => 'r',
            Self::OpenProject => 'p',
            Self::Help => '?',
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

    let logo_width = LOGO.iter().map(|line| line.len()).max().unwrap_or(0) as u16;
    let show_logo = area.width >= logo_width && area.height >= 20;
    let wide = area.width >= WIDE_LAYOUT_WIDTH && area.height >= 19;
    let heading_height: u16 = if show_logo { LOGO.len() as u16 } else { 1 };
    let menu_height = if wide { 6 } else { dashboard.actions.len() as u16 + 3 };
    let content_height = heading_height + 3 + menu_height;
    let top = area.y + area.height.saturating_sub(content_height) / 2;

    // A row renderer that never extends beyond the visible terminal.
    let mut draw_line = |line: &str, x: u16, row: u16, width: u16, style| {
        if row >= area.bottom() || x >= area.right() {
            return;
        }
        let width = width.min(area.right().saturating_sub(x));
        if width == 0 {
            return;
        }
        let text = Text::from(line);
        Paragraph::new(&text)
            .style(style)
            .render(Rect::new(x, row, width, 1), surface);
    };

    if show_logo {
        for (index, line) in LOGO.iter().enumerate() {
            let width = line.len() as u16;
            let x = area.x + area.width.saturating_sub(width) / 2;
            draw_line(line, x, top + index as u16, width, accent);
        }
    } else {
        let x = area.x + area.width.saturating_sub(6) / 2;
        draw_line("PRIMER", x, top, 6, accent);
    }

    let subtitle = "P R I M E R";
    let subtitle_x = area.x + area.width.saturating_sub(subtitle.len() as u16) / 2;
    draw_line(subtitle, subtitle_x, top + heading_height, subtitle.len() as u16, accent);

    let menu_top = top + heading_height + 3;
    if wide {
        // Match the two-column dashboard rhythm of the Neovim setup.
        // The first two actions are on the left; the remaining two on the right.
        let total_width = COLUMN_WIDTH * 2 + COLUMN_GAP;
        let left = area.x + area.width.saturating_sub(total_width) / 2;
        let right = left + COLUMN_WIDTH + COLUMN_GAP;
        draw_line("FILES", left, menu_top, COLUMN_WIDTH, accent);
        draw_line("WORKSPACE", right, menu_top, COLUMN_WIDTH, accent);

        for (index, action) in dashboard.actions.iter().enumerate() {
            let column = index / 2;
            let row = index % 2;
            let x = if column == 0 { left } else { right };
            let marker = if index == dashboard.selected { ">" } else { " " };
            let label = format!("{marker}  {}  {}", action.shortcut(), action.label());
            let style = if index == dashboard.selected { selected } else { normal };
            draw_line(&label, x, menu_top + 2 + row as u16, COLUMN_WIDTH, style);
        }
        draw_line("j/k navigate  Enter select  Esc edit", left, menu_top + 5, total_width, normal);
    } else {
        for (index, action) in dashboard.actions.iter().enumerate() {
            let marker = if index == dashboard.selected { ">" } else { " " };
            let label = format!("{marker}  {}  {}", action.shortcut(), action.label());
            let width = (label.len() as u16).min(area.width);
            let x = area.x + area.width.saturating_sub(width) / 2;
            let style = if index == dashboard.selected { selected } else { normal };
            draw_line(&label, x, menu_top + index as u16, width, style);
        }
    }
}

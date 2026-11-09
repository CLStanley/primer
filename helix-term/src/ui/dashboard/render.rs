use helix_view::graphics::Rect;
use tui::buffer::Buffer as Surface;
use tui::text::Text;
use tui::widgets::{Paragraph, Widget};

use super::{Dashboard, DashboardAction};
use crate::compositor::Context;

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
    let normal = ctx.editor.theme.get("ui.text");
    let selected = ctx.editor.theme.get("ui.selection");
    surface.clear_with(area, normal);

    let width = 34_u16.min(area.width);
    let x = area.x + area.width.saturating_sub(width) / 2;
    let content_height = dashboard.actions.len() as u16 + 5;
    let y = area.y + area.height.saturating_sub(content_height) / 2;

    let draw_line = |line: &str, row: u16, style| {
        if row >= area.bottom() || width == 0 {
            return;
        }
        let text = Text::from(line);
        Paragraph::new(&text)
            .style(style)
            .render(Rect::new(x, row, width, 1), surface);
    };

    draw_line("             PRIMER", y, normal);
    for (index, action) in dashboard.actions.iter().enumerate() {
        let row = y.saturating_add(2).saturating_add(index as u16);
        let marker = if index == dashboard.selected { ">" } else { " " };
        let label = format!("  {marker}  {}", action.label());
        let style = if index == dashboard.selected { selected } else { normal };
        draw_line(&label, row, style);
    }
    draw_line(
        "  j/k: navigate   Enter: open   Esc: edit",
        y.saturating_add(content_height.saturating_sub(1)),
        normal,
    );
}

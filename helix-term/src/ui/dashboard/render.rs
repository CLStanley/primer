use helix_view::graphics::{Color, Modifier, Rect, Style};
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
            Self::NewFile => "New file",
            Self::Quit => "Quit",
        }
    }

    pub fn shortcut(self) -> char {
        match self {
            Self::FindFile => 'f',
            Self::RecentFiles => 'r',
            Self::OpenProject => 'p',
            Self::Help => '?',
            Self::NewFile => 'n',
            Self::Quit => 'q',
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

    // Use terminal-default backgrounds throughout, including the padding
    // painted by Paragraph. This preserves terminal transparency.
    let theme = &ctx.editor.theme;
    let text_color = theme.get("ui.text").fg.unwrap_or(Color::Reset);
    // Theme scopes rather than fixed RGB values keep the dashboard aligned
    // with the user's chosen theme. Primer's default heading color is lilac.
    let accent_color = theme
        .try_get("markup.heading")
        .and_then(|style| style.fg)
        .or_else(|| theme.try_get("ui.linenr.selected").and_then(|style| style.fg))
        .unwrap_or(text_color);
    // Separate foreground roles mirror the Neovim dashboard: purple
    // headings, blue shortcuts, and bright descriptions.
    let description_color = theme
        .try_get("ui.text.focus")
        .and_then(|style| style.fg)
        .unwrap_or(text_color);
    let shortcut_color = theme
        .try_get("diagnostic.info")
        .and_then(|style| style.underline_color.or(style.fg))
        .unwrap_or(Color::LightBlue);
    let green_color = theme
        .try_get("diagnostic.hint")
        .and_then(|style| style.underline_color.or(style.fg))
        .or_else(|| theme.try_get("variable.builtin").and_then(|style| style.fg))
        .unwrap_or(Color::LightGreen);
    let normal = Style::default().fg(description_color).bg(Color::Reset);
    let accent = Style::default().fg(accent_color).bg(Color::Reset);
    let shortcut = Style::default().fg(shortcut_color).bg(Color::Reset);
    let selected = normal.add_modifier(Modifier::BOLD);
    let selected_shortcut = shortcut.add_modifier(Modifier::BOLD);
    let green = Style::default().fg(green_color).bg(Color::Reset);
    let pending = Style::default().fg(text_color).bg(Color::Reset).add_modifier(Modifier::DIM);
    surface.clear_with(area, Style::reset());

    let logo_width = LOGO.iter().map(|line| line.len()).max().unwrap_or(0) as u16;
    let show_logo = area.width >= logo_width && area.height >= 20;
    let wide = area.width >= WIDE_LAYOUT_WIDTH && area.height >= 19;
    let heading_height: u16 = if show_logo { LOGO.len() as u16 } else { 1 };
    let menu_height = if wide { 11 } else { dashboard.actions.len() as u16 + 3 };
    let content_height = heading_height + 5 + menu_height;
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
            draw_line(line, x, top + index as u16, width, shortcut);
        }
    } else {
        let x = area.x + area.width.saturating_sub(6) / 2;
        draw_line("PRIMER", x, top, 6, shortcut);
    }

    let motto = "Leaves from the vine...";
    if area.width >= motto.len() as u16 {
        let motto_x = area.x + (area.width - motto.len() as u16) / 2;
        draw_line(motto, motto_x, top + heading_height + 2, motto.len() as u16, green);
    }

    let menu_top = top + heading_height + 5;
    if wide {
        // Match the two-column dashboard rhythm of the Neovim setup.
        // The first two actions are on the left; the remaining two on the right.
        let total_width = COLUMN_WIDTH * 2 + COLUMN_GAP;
        let left = area.x + area.width.saturating_sub(total_width) / 2;
        let right = left + COLUMN_WIDTH + COLUMN_GAP;
        // Center each heading above its column, not against its left edge.
        draw_line("FILES", left + (COLUMN_WIDTH - 5) / 2, menu_top, 5, accent);
        draw_line("WORKSPACE", right + (COLUMN_WIDTH - 9) / 2, menu_top, 9, accent);

        for (index, action) in dashboard.actions.iter().enumerate().take(4) {
            let column = index / 2;
            let row = index % 2;
            let x = if column == 0 { left } else { right };
            let y = menu_top + 2 + row as u16;
            let active = index == dashboard.selected;
            let marker = if active { ">" } else { " " };
            draw_line(marker, x, y, 1, if active { accent } else { normal });
            let key = action.shortcut().to_string();
            draw_line(&key, x + 3, y, 1, if active { selected_shortcut } else { shortcut });
            draw_line(action.label(), x + 6, y, COLUMN_WIDTH - 6, if active { selected } else { normal });
        }

        // Roadmap entries are intentionally non-interactive until implemented.
        // They make the missing features visible without pretending to work.
        for (x, y, key, label) in [
            (left, menu_top + 4, "t", "Terminal [planned]"),
            (right, menu_top + 4, "s", "Sessions [planned]"),
            (left, menu_top + 5, "g", "Live grep [planned]"),
            (right, menu_top + 5, "o", "Open project [planned]"),
        ] {
            draw_line(key, x + 3, y, 1, shortcut);
            draw_line(label, x + 6, y, COLUMN_WIDTH - 6, pending);
        }

        for (action, x) in [
            (DashboardAction::NewFile, left),
            (DashboardAction::Quit, right),
        ] {
            let active = dashboard.selected_action() == Some(action);
            let key = action.shortcut().to_string();
            draw_line(&key, x + 3, menu_top + 8, 1, if active { selected_shortcut } else { shortcut });
            draw_line(action.label(), x + 6, menu_top + 8, COLUMN_WIDTH - 6, if active { selected } else { normal });
        }

        // Draw shortcut hints in blue and descriptions in white.
        let hint = [("j/k", " navigate  "), ("Enter", " select")];
        let hint_width: u16 = hint.iter().map(|(key, label)| (key.len() + label.len()) as u16).sum();
        let mut hint_x = left + total_width.saturating_sub(hint_width) / 2;
        for (key, label) in hint {
            draw_line(key, hint_x, menu_top + 10, key.len() as u16, shortcut);
            hint_x += key.len() as u16;
            draw_line(label, hint_x, menu_top + 10, label.len() as u16, normal);
            hint_x += label.len() as u16;
        }
    } else {
        for (index, action) in dashboard.actions.iter().enumerate() {
            let y = menu_top + index as u16;
            let x = area.x + area.width.saturating_sub(COLUMN_WIDTH) / 2;
            let active = index == dashboard.selected;
            draw_line(if active { ">" } else { " " }, x, y, 1, accent);
            let key = action.shortcut().to_string();
            draw_line(&key, x + 3, y, 1, if active { selected_shortcut } else { shortcut });
            draw_line(action.label(), x + 6, y, COLUMN_WIDTH - 6, if active { selected } else { normal });
        }
    }
}

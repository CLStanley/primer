use helix_view::graphics::{Color, Modifier, Rect, Style};
use tui::buffer::Buffer as Surface;
use tui::text::Text;
use tui::widgets::{Paragraph, Widget};

use super::{Dashboard, DashboardAction};
use crate::compositor::Context;

// Five-line PRIMER ASCII logo.
// Keep whitespace intact to preserve the artwork.
const LOGO: [&str; 5] = [
    "  ____  ____  ___ __  __ _____ ____  ",
    " |  _ \\|  _ \\|_ _|  \\/  | ____|  _ \\ ",
    " | |_) | |_) || || |\\/| |  _| | |_) |",
    " |  __/|  _ < | || |  | | |___|  _ < ",
    " |_|   |_| \\_\\___|_|  |_|_____|_| \\_\\",
];
const WIDE_LAYOUT_WIDTH: u16 = 76;
const COLUMN_GAP: u16 = 8;
const COLUMN_WIDTH: u16 = 30;

impl DashboardAction {
    pub fn label(self) -> &'static str {
        match self {
            Self::FindFile => "Browse files",
            Self::LiveGrep => "Live grep",
            Self::FindNotes => "Find notes",
            Self::NewNote => "New note",
            Self::DailyNote => "Daily note",
            Self::Agenda => "Agenda",
            Self::FileExplorer => "File explorer",
            Self::OpenProject => "Browse workspace",
            Self::Help => "Keybind help",
            Self::NewFile => "New file",
            Self::Quit => "Quit",
        }
    }

    pub fn shortcut(self) -> char {
        match self {
            Self::FindFile => 'f',
            Self::LiveGrep => 'g',
            Self::FindNotes => 'o',
            Self::NewNote => 'a',
            Self::DailyNote => 'd',
            Self::Agenda => 'e',
            Self::FileExplorer => 'r',
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
        .try_get("ui.dashboard.heading")
        .and_then(|style| style.fg)
        .or_else(|| {
            theme
                .try_get("ui.linenr.selected")
                .and_then(|style| style.fg)
        })
        .unwrap_or(text_color);
    // Separate foreground roles mirror the Neovim dashboard: purple
    // headings, blue shortcuts, and bright descriptions.
    let description_color = theme
        .try_get("ui.text.focus")
        .and_then(|style| style.fg)
        .unwrap_or(text_color);
    let shortcut_color = theme
        .try_get_exact("ui.dashboard.shortcut")
        .and_then(|style| style.fg)
        .unwrap_or(Color::Rgb(86, 200, 255));
    let green_color = theme
        .try_get_exact("ui.dashboard.motto")
        .and_then(|style| style.fg)
        .unwrap_or(Color::Rgb(159, 242, 143));
    let normal = Style::default().fg(description_color).bg(Color::Reset);
    let accent = Style::default().fg(accent_color).bg(Color::Reset);
    let shortcut = Style::default().fg(shortcut_color).bg(Color::Reset);
    let logo_color = theme
        .try_get_exact("ui.dashboard.logo")
        .and_then(|style| style.fg)
        .unwrap_or(accent_color);
    let logo_style = Style::default()
        .fg(logo_color)
        .bg(Color::Reset)
        .add_modifier(Modifier::BOLD);
    let selected = normal.add_modifier(Modifier::BOLD);
    let selected_shortcut = shortcut.add_modifier(Modifier::BOLD);
    let green = Style::default().fg(green_color).bg(Color::Reset);
    let pending = Style::default()
        .fg(text_color)
        .bg(Color::Reset)
        .add_modifier(Modifier::DIM);
    surface.clear_with(area, Style::reset());

    let logo_width = LOGO.iter().map(|line| line.len()).max().unwrap_or(0) as u16;
    let show_logo = area.width >= logo_width && area.height >= 20;
    let wide = area.width >= WIDE_LAYOUT_WIDTH && area.height >= 19;
    let heading_height: u16 = if show_logo { LOGO.len() as u16 } else { 1 };
    let menu_height = if wide { 21 } else { 29 };
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
        // ASCII art is one fixed-width block. Centering individual rows
        // shifts their strokes whenever line lengths differ.
        let logo_x = area.x + area.width.saturating_sub(logo_width) / 2;
        for (index, line) in LOGO.iter().enumerate() {
            draw_line(line, logo_x, top + index as u16, logo_width, logo_style);
        }
    } else {
        let x = area.x + area.width.saturating_sub(6) / 2;
        draw_line("PRIMER", x, top, 6, logo_style);
    }

    let motto = "Leaves from the vine...";
    if area.width >= motto.len() as u16 {
        let motto_x = area.x + (area.width - motto.len() as u16) / 2;
        draw_line(
            motto,
            motto_x,
            top + heading_height + 2,
            motto.len() as u16,
            green,
        );
    }

    let menu_top = top + heading_height + 5;
    // Entries are the dashboard roadmap. A missing action is deliberately
    // displayed as [todo] and never receives a key handler.
    // The two-column layout uses two rows of sections.
    let sections: [(&str, Vec<(&str, &str, Option<DashboardAction>)>); 4] = [
        (
            "FILES",
            vec![
                ("f", "Browse files", Some(DashboardAction::FindFile)),
                ("r", "File explorer", Some(DashboardAction::FileExplorer)),
                ("n", "New file", Some(DashboardAction::NewFile)),
                ("g", "Live grep", Some(DashboardAction::LiveGrep)),
            ],
        ),
        (
            "PROJECTS",
            vec![
                ("p", "Browse workspace", Some(DashboardAction::OpenProject)),
                ("s", "Switch project", None),
                ("w", "Sessions", None),
                ("l", "Recent projects", None),
            ],
        ),
        (
            "NOTES",
            vec![
                ("a", "New note", Some(DashboardAction::NewNote)),
                ("d", "Daily note", Some(DashboardAction::DailyNote)),
                ("o", "Find notes", Some(DashboardAction::FindNotes)),
                ("e", "Agenda", Some(DashboardAction::Agenda)),
            ],
        ),
        (
            "REMOTE SSH",
            vec![
                ("c", "Connect to host", None),
                ("h", "Saved hosts", None),
                ("u", "Remote files", None),
                ("x", "Remote terminal", None),
            ],
        ),
    ];

    if wide {
        let total_width = COLUMN_WIDTH * 2 + COLUMN_GAP;
        let left = area.x + area.width.saturating_sub(total_width) / 2;
        let right = left + COLUMN_WIDTH + COLUMN_GAP;
        for (section_index, (heading, entries)) in sections.iter().enumerate() {
            let x = if section_index % 2 == 0 { left } else { right };
            let section_top = menu_top + (section_index / 2) as u16 * 7;
            let heading_width = heading.len() as u16;
            draw_line(
                heading,
                x + (COLUMN_WIDTH - heading_width) / 2,
                section_top,
                heading_width,
                accent,
            );
            for (row, (key, label, action)) in entries.iter().enumerate() {
                let y = section_top + 2 + row as u16;
                let active = action.is_some() && *action == dashboard.selected_action();
                if active {
                    draw_line(">", x, y, 1, accent);
                }
                draw_line(
                    key,
                    x + 3,
                    y,
                    1,
                    if active { selected_shortcut } else { shortcut },
                );
                let description = if action.is_some() {
                    label.to_string()
                } else {
                    format!("{label} [todo]")
                };
                draw_line(
                    &description,
                    x + 6,
                    y,
                    COLUMN_WIDTH - 6,
                    if active {
                        selected
                    } else if action.is_some() {
                        normal
                    } else {
                        pending
                    },
                );
            }
        }
        let footer_y = menu_top + 16;
        // Shared actions form one evenly spaced horizontal footer.
        // Center each footer item in an equal-width third of the menu.
        // This keeps the spacing balanced regardless of label length.
        let footer_width = total_width + 20;
        let footer_left = area.x + area.width.saturating_sub(footer_width) / 2;
        let third = footer_width / 3;
        let footer = [
            ("t", "Terminal [todo]", None),
            ("?", "Keybind help", Some(DashboardAction::Help)),
            ("q", "Quit", Some(DashboardAction::Quit)),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (key, label, action))| {
            let item_width = 2 + label.len() as u16;
            let x = footer_left + index as u16 * third + third.saturating_sub(item_width) / 2;
            (x, key, label, action)
        });
        for (x, key, label, action) in footer {
            let active = action.is_some() && action == dashboard.selected_action();
            draw_line(
                key,
                x,
                footer_y,
                1,
                if active { selected_shortcut } else { shortcut },
            );
            draw_line(
                label,
                x + 2,
                footer_y,
                label.len() as u16,
                if active {
                    selected
                } else if action.is_some() {
                    normal
                } else {
                    pending
                },
            );
        }
        let hint = [("h/j/k/l", " navigate  "), ("Enter", " select")];
        let hint_width: u16 = hint
            .iter()
            .map(|(key, label)| (key.len() + label.len()) as u16)
            .sum();
        let mut hint_x = left + total_width.saturating_sub(hint_width) / 2;
        for (key, label) in hint {
            draw_line(key, hint_x, menu_top + 19, key.len() as u16, shortcut);
            hint_x += key.len() as u16;
            draw_line(label, hint_x, menu_top + 19, label.len() as u16, normal);
            hint_x += label.len() as u16;
        }
    } else {
        let x = area.x + area.width.saturating_sub(COLUMN_WIDTH) / 2;
        let mut y = menu_top;
        for (heading, entries) in &sections {
            draw_line(heading, x, y, heading.len() as u16, accent);
            y += 1;
            for (key, label, action) in entries {
                let active = action.is_some() && *action == dashboard.selected_action();
                if active {
                    draw_line(">", x, y, 1, accent);
                }
                draw_line(key, x + 2, y, 1, shortcut);
                let description = if action.is_some() {
                    label.to_string()
                } else {
                    format!("{label} [todo]")
                };
                draw_line(
                    &description,
                    x + 4,
                    y,
                    COLUMN_WIDTH - 4,
                    if active {
                        selected
                    } else if action.is_some() {
                        normal
                    } else {
                        pending
                    },
                );
                y += 1;
            }
            y += 1;
        }
        for (key, label, action) in [
            ("t", "Terminal [todo]", None),
            ("?", "Keybind help", Some(DashboardAction::Help)),
            ("q", "Quit", Some(DashboardAction::Quit)),
        ] {
            let active = action.is_some() && action == dashboard.selected_action();
            draw_line(key, x + 2, y, 1, shortcut);
            draw_line(
                label,
                x + 4,
                y,
                COLUMN_WIDTH - 4,
                if active {
                    selected
                } else if action.is_some() {
                    normal
                } else {
                    pending
                },
            );
            y += 1;
        }
    }
}

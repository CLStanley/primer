mod render;

use crate::compositor::{Component, Context, Event, EventResult};
use crate::key;
use crate::commands::{self, MappableCommand};
use helix_view::graphics::Rect;
use tui::buffer::Buffer as Surface;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DashboardAction {
    FindFile,
    LiveGrep,
    RecentFiles,
    OpenProject,
    Help,
    NewFile,
    Quit,
}

pub struct Dashboard {
    pub selected: usize,
    pub actions: Vec<DashboardAction>,
}

impl Dashboard {
    pub fn new() -> Self {
        Self {
            selected: 0,
            actions: vec![
                DashboardAction::FindFile,
                DashboardAction::RecentFiles,
                DashboardAction::LiveGrep,
                DashboardAction::NewFile,
                DashboardAction::OpenProject,
                DashboardAction::Help,
                DashboardAction::Quit,
            ],
        }
    }

    // Navigation follows the visible two-column arrangement, not the
    // declaration order of the action enum.
    pub fn next(&mut self) {
        self.select_neighbor(true, false);
    }

    pub fn previous(&mut self) {
        self.select_neighbor(false, false);
    }

    pub fn left(&mut self) {
        self.select_neighbor(false, true);
    }

    pub fn right(&mut self) {
        self.select_neighbor(true, true);
    }

    fn select_neighbor(&mut self, forward: bool, horizontal: bool) {
        let Some(current) = self.selected_action() else { return };
        let target = if horizontal {
            match current {
                DashboardAction::FindFile | DashboardAction::RecentFiles | DashboardAction::NewFile | DashboardAction::LiveGrep =>
                    DashboardAction::OpenProject,
                DashboardAction::OpenProject => DashboardAction::FindFile,
                DashboardAction::Help => DashboardAction::Quit,
                DashboardAction::Quit => DashboardAction::Help,
            }
        } else {
            let column: &[DashboardAction] = match current {
                DashboardAction::FindFile | DashboardAction::RecentFiles | DashboardAction::NewFile | DashboardAction::LiveGrep |
                DashboardAction::Help => &[
                    DashboardAction::FindFile, DashboardAction::RecentFiles,
                    DashboardAction::NewFile, DashboardAction::LiveGrep, DashboardAction::Help,
                ],
                DashboardAction::OpenProject | DashboardAction::Quit => &[
                    DashboardAction::OpenProject, DashboardAction::Quit,
                ],
            };
            let index = column.iter().position(|action| *action == current).unwrap_or(0);
            let next = if forward {
                (index + 1) % column.len()
            } else {
                (index + column.len() - 1) % column.len()
            };
            column[next]
        };
        if let Some(index) = self.actions.iter().position(|action| *action == target) {
            self.selected = index;
        }
    }

    pub fn selected_action(&self) -> Option<DashboardAction> {
        self.actions.get(self.selected).copied()
    }
}

impl Component for Dashboard {
    fn render(&mut self, area: Rect, surface: &mut Surface, _ctx: &mut Context) {
        render::render_dashboard(self, area, surface, _ctx);
    }

    fn handle_event(&mut self, event: &Event, _ctx: &mut Context) -> EventResult {
        let key = match event {
            Event::Key(key) => *key,
            Event::Mouse(_) => return EventResult::Consumed(None),
            _ => return EventResult::Ignored(None),
        };

        match key {
            key!('j') | key!(Down) => {
                self.next();
                EventResult::Consumed(None)
            }
            key!('k') | key!(Up) => {
                self.previous();
                EventResult::Consumed(None)
            }
            key!('h') | key!(Left) => {
                self.left();
                EventResult::Consumed(None)
            }
            key!('l') | key!(Right) => {
                self.right();
                EventResult::Consumed(None)
            }
            // The dashboard is the startup screen. Dismissing it into an
            // unnamed buffer must be an intentional New file action.
            key!(Esc) => EventResult::Consumed(None),
            key!(Enter) | key!('f') | key!('r') | key!('g') | key!('p') | key!('?') | key!('n') | key!('q') => {
                let action = match key {
                    key!('f') => Some(DashboardAction::FindFile),
                    key!('r') => Some(DashboardAction::RecentFiles),
                    key!('g') => Some(DashboardAction::LiveGrep),
                    key!('p') => Some(DashboardAction::OpenProject),
                    key!('?') => Some(DashboardAction::Help),
                    key!('n') => Some(DashboardAction::NewFile),
                    key!('q') => Some(DashboardAction::Quit),
                    _ => self.selected_action(),
                };
                EventResult::Consumed(Some(Box::new(move |compositor, cx| {
                    // Keep the dashboard beneath pickers so cancelling returns home.
                    // It is dismissed after a file is successfully opened.
                    if action == Some(DashboardAction::NewFile) {
                        compositor.remove_type::<Dashboard>();
                        return;
                    }
                    if matches!(action, Some(DashboardAction::Quit)) {
                        compositor.remove_type::<Dashboard>();
                    }
                    let command = match action {
                        Some(DashboardAction::FindFile) => MappableCommand::file_picker,
                        Some(DashboardAction::LiveGrep) => MappableCommand::global_search,
                        Some(DashboardAction::RecentFiles) => MappableCommand::buffer_picker,
                        Some(DashboardAction::OpenProject) => {
                            MappableCommand::file_picker_in_current_directory
                        }
                        Some(DashboardAction::Help) => MappableCommand::command_palette,
                        Some(DashboardAction::NewFile) => return,
                        Some(DashboardAction::Quit) => MappableCommand::Typable {
                            name: "quit-all".to_owned(),
                            args: String::new(),
                            doc: "Close all views".to_owned(),
                        },
                        None => return,
                    };
                    let callbacks = {
                        let mut command_cx = commands::Context {
                            register: None,
                            count: None,
                            editor: &mut *cx.editor,
                            callback: Vec::new(),
                            on_next_key_callback: None,
                            jobs: &mut *cx.jobs,
                        };
                        command.execute(&mut command_cx);
                        command_cx.callback
                    };
                    for callback in callbacks {
                        callback(compositor, cx);
                    }
                })))
            }
            _ => EventResult::Consumed(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_on_browse_files() {
        let dashboard = Dashboard::new();
        assert_eq!(dashboard.selected_action(), Some(DashboardAction::FindFile));
        assert_eq!(dashboard.actions.len(), 7);
    }

    #[test]
    fn vertical_navigation_stays_in_column_and_wraps() {
        let mut dashboard = Dashboard::new();
        dashboard.next();
        assert_eq!(dashboard.selected_action(), Some(DashboardAction::RecentFiles));
        dashboard.next();
        assert_eq!(dashboard.selected_action(), Some(DashboardAction::NewFile));
        dashboard.next();
        assert_eq!(dashboard.selected_action(), Some(DashboardAction::LiveGrep));
        dashboard.next();
        assert_eq!(dashboard.selected_action(), Some(DashboardAction::Help));
        dashboard.next();
        assert_eq!(dashboard.selected_action(), Some(DashboardAction::FindFile));
        dashboard.previous();
        assert_eq!(dashboard.selected_action(), Some(DashboardAction::Help));
    }

    #[test]
    fn horizontal_navigation_switches_columns() {
        let mut dashboard = Dashboard::new();
        dashboard.right();
        assert_eq!(dashboard.selected_action(), Some(DashboardAction::OpenProject));
        dashboard.next();
        assert_eq!(dashboard.selected_action(), Some(DashboardAction::Quit));
        dashboard.left();
        assert_eq!(dashboard.selected_action(), Some(DashboardAction::Help));
        dashboard.right();
        assert_eq!(dashboard.selected_action(), Some(DashboardAction::Quit));
    }

    #[test]
    fn invalid_selection_is_safe() {
        let mut dashboard = Dashboard::new();
        dashboard.selected = 100;
        dashboard.next();
        dashboard.right();
        assert_eq!(dashboard.selected_action(), None);
    }
}

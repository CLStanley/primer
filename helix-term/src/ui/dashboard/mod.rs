mod render;

use crate::compositor::{Component, Context, Event, EventResult};
use crate::key;
use helix_view::graphics::Rect;
use tui::buffer::Buffer as Surface;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DashboardAction {
    FindFile,
    RecentFiles,
    OpenProject,
    Help,
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
                DashboardAction::OpenProject,
                DashboardAction::Help,
            ],
        }
    }

    pub fn next(&mut self) {
        if !self.actions.is_empty() {
            self.selected = (self.selected + 1) % self.actions.len();
        }
    }

    pub fn previous(&mut self) {
        if !self.actions.is_empty() {
            self.selected = if self.selected == 0 {
                self.actions.len() - 1
            } else {
                self.selected - 1
            };
        }
    }

    pub fn selected_action(&self) -> Option<DashboardAction> {
        self.actions.get(self.selected).copied()
    }
}

impl Component for Dashboard {
    fn render(&mut self, area: Rect, surface: &mut Surface, _ctx: &mut Context) {
        render::render_dashboard(self, area, surface);
    }

    fn handle_event(&mut self, event: &Event, _ctx: &mut Context) -> EventResult {
        match event {
            Event::Key(key) => match *key {
                key!('j') | key!(Down) => {
                    self.next();
                    EventResult::Consumed(None)
                }
                key!('k') | key!(Up) => {
                    self.previous();
                    EventResult::Consumed(None)
                }
                _ => EventResult::Ignored(None),
            },
            _ => EventResult::Ignored(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashboard_starts_with_first_action_selected() {
        let dashboard = Dashboard::new();

        assert_eq!(dashboard.selected, 0);
        assert_eq!(dashboard.actions.len(), 4);
        assert_eq!(dashboard.actions[0], DashboardAction::FindFile);
    }

    #[test]
    fn dashboard_next_wraps_to_first_action() {
        let mut dashboard = Dashboard::new();

        dashboard.next();
        assert_eq!(dashboard.selected, 1);

        dashboard.next();
        assert_eq!(dashboard.selected, 2);

        dashboard.next();
        assert_eq!(dashboard.selected, 3);

        dashboard.next();
        assert_eq!(dashboard.selected, 0);
    }

    #[test]
    fn dashboard_previous_wraps_to_last_action() {
        let mut dashboard = Dashboard::new();

        dashboard.previous();
        assert_eq!(dashboard.selected, 3);

        dashboard.previous();
        assert_eq!(dashboard.selected, 2);
    }

    #[test]
    fn selected_action_returns_initial_action() {
        let dashboard = Dashboard::new();

        assert_eq!(dashboard.selected_action(), Some(DashboardAction::FindFile));
    }

    #[test]
    fn selected_action_updates_after_navigation() {
        let mut dashboard = Dashboard::new();

        dashboard.next();

        assert_eq!(
            dashboard.selected_action(),
            Some(DashboardAction::RecentFiles)
        );

        dashboard.next();

        assert_eq!(
            dashboard.selected_action(),
            Some(DashboardAction::OpenProject)
        );
    }

    #[test]
    fn selected_action_returns_none_for_invalid_index() {
        let mut dashboard = Dashboard::new();

        dashboard.selected = 100;

        assert_eq!(dashboard.selected_action(), None);
    }
}

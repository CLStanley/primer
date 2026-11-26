mod render;

use crate::commands::{self, MappableCommand};
use crate::compositor::{Component, Context, Event, EventResult};
use crate::key;
use helix_view::graphics::Rect;
use tui::buffer::Buffer as Surface;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DashboardAction {
    FindFile,
    LiveGrep,
    FindNotes,
    NewNote,
    DailyNote,
    Agenda,
    FileExplorer,
    OpenProject,
    AddProject,
    ForgetProject,
    ClearProject,
    ProjectStatus,
    Help,
    NewFile,
    Quit,
    Terminal,
    LazyGit,
    LazyDocker,
    RemoteConnect,
    RemoteConnections,
    RemoteBrowse,
    RemoteDisconnect,
    RemoteDirectory,
    RemoteSavedHosts,
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
                DashboardAction::FileExplorer,
                DashboardAction::LiveGrep,
                DashboardAction::FindNotes,
                DashboardAction::NewNote,
                DashboardAction::DailyNote,
                DashboardAction::Agenda,
                DashboardAction::NewFile,
                DashboardAction::OpenProject,
                DashboardAction::AddProject,
                DashboardAction::ForgetProject,
                DashboardAction::ClearProject,
                DashboardAction::ProjectStatus,
                DashboardAction::Help,
                DashboardAction::Quit,
                DashboardAction::Terminal,
                DashboardAction::LazyGit,
                DashboardAction::LazyDocker,
                DashboardAction::RemoteConnect,
                DashboardAction::RemoteConnections,
                DashboardAction::RemoteDisconnect,
                DashboardAction::RemoteDirectory,
                DashboardAction::RemoteSavedHosts,
            ],
        }
    }

    fn refresh_actions(&mut self) {
        let selected = self.selected_action();
        let mut actions = Self::new().actions;
        #[cfg(target_os = "linux")]
        if commands::ssh_dashboard_indicator().is_some() {
            actions.retain(|action| !matches!(action, DashboardAction::LazyGit | DashboardAction::LazyDocker));
        }
        self.actions = actions;
        self.selected = selected.and_then(|action| self.actions.iter().position(|candidate| *candidate == action)).unwrap_or(0);
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
        let Some(current) = self.selected_action() else {
            return;
        };
        let target = if horizontal {
            match current {
                DashboardAction::FindFile
                | DashboardAction::FileExplorer
                | DashboardAction::NewFile
                | DashboardAction::LiveGrep
                | DashboardAction::FindNotes
                | DashboardAction::NewNote
                | DashboardAction::DailyNote
                | DashboardAction::Agenda => DashboardAction::OpenProject,
                DashboardAction::OpenProject | DashboardAction::AddProject | DashboardAction::ForgetProject | DashboardAction::ClearProject | DashboardAction::ProjectStatus | DashboardAction::RemoteConnect | DashboardAction::RemoteConnections | DashboardAction::RemoteBrowse | DashboardAction::RemoteDisconnect | DashboardAction::RemoteDirectory | DashboardAction::RemoteSavedHosts => DashboardAction::FindFile,
                DashboardAction::Terminal | DashboardAction::LazyGit | DashboardAction::LazyDocker | DashboardAction::Help | DashboardAction::Quit => {
                    let footer: Vec<_> = [DashboardAction::Terminal, DashboardAction::LazyGit, DashboardAction::LazyDocker, DashboardAction::Help, DashboardAction::Quit].into_iter().filter(|action| self.actions.contains(action)).collect();
                    let i = footer.iter().position(|a| *a == current).unwrap_or(0);
                    footer[if forward { (i + 1) % footer.len() } else { (i + footer.len() - 1) % footer.len() }]
                },
            }
        } else {
            let column: &[DashboardAction] = match current {
                DashboardAction::FindFile
                | DashboardAction::FileExplorer
                | DashboardAction::NewFile
                | DashboardAction::LiveGrep
                | DashboardAction::FindNotes
                | DashboardAction::NewNote
                | DashboardAction::DailyNote
                | DashboardAction::Agenda
                | DashboardAction::Terminal | DashboardAction::LazyGit | DashboardAction::LazyDocker | DashboardAction::Help => &[
                    DashboardAction::FindFile,
                    DashboardAction::FileExplorer,
                    DashboardAction::NewFile,
                    DashboardAction::LiveGrep,
                    DashboardAction::NewNote,
                    DashboardAction::DailyNote,
                    DashboardAction::FindNotes,
                    DashboardAction::Agenda,
                    DashboardAction::Help,
                    DashboardAction::Terminal,
                    DashboardAction::LazyGit,
                    DashboardAction::LazyDocker,
                ],
                DashboardAction::OpenProject | DashboardAction::AddProject | DashboardAction::ForgetProject | DashboardAction::ClearProject | DashboardAction::ProjectStatus | DashboardAction::RemoteConnect | DashboardAction::RemoteConnections | DashboardAction::RemoteBrowse | DashboardAction::RemoteDisconnect | DashboardAction::RemoteDirectory | DashboardAction::RemoteSavedHosts | DashboardAction::Quit => {
                    &[DashboardAction::OpenProject, DashboardAction::AddProject, DashboardAction::ForgetProject, DashboardAction::ClearProject, DashboardAction::ProjectStatus, DashboardAction::RemoteConnect, DashboardAction::RemoteConnections, DashboardAction::RemoteBrowse, DashboardAction::RemoteDisconnect, DashboardAction::RemoteDirectory, DashboardAction::RemoteSavedHosts, DashboardAction::Quit]
                }
            };
            let column: Vec<_> = column.iter().copied().filter(|action| self.actions.contains(action)).collect();
            let index = column
                .iter()
                .position(|action| *action == current)
                .unwrap_or(0);
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
        self.refresh_actions();
        render::render_dashboard(self, area, surface, _ctx);
    }

    fn handle_event(&mut self, event: &Event, _ctx: &mut Context) -> EventResult {
        self.refresh_actions();
        let key = match event {
            Event::Key(key) => *key,
            Event::Mouse(_) => return EventResult::Consumed(None),
            _ => return EventResult::Ignored(None),
        };

        // Backends may report uppercase letters with an explicit Shift modifier.
        // Normalize these two shortcuts before matching the dashboard bindings.
        let key = if key.modifiers == helix_view::keyboard::KeyModifiers::SHIFT {
            match key.code {
                helix_view::keyboard::KeyCode::Char('g' | 'G') => key!('G'),
                helix_view::keyboard::KeyCode::Char('d' | 'D') => key!('D'),
                _ => key,
            }
        } else {
            key
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
            key!(Enter)
            | key!('f')
            | key!('r')
            | key!('g')
            | key!('o')
            | key!('a')
            | key!('d')
            | key!('e')
            | key!('p')
            | key!('b')
            | key!('x')
            | key!('c')
            | key!('s')
            | key!('?')
            | key!('n')
            | key!('q')
            | key!('z') | key!('y') | key!('w') | key!('v') | key!('u') | key!('i') | key!('t') | key!('G') | key!('D') => {
                let action = match key {
                    key!('z') => Some(DashboardAction::RemoteSavedHosts),
                    key!('y') => Some(DashboardAction::RemoteDirectory),
                    key!('w') => Some(DashboardAction::RemoteDisconnect),
                    key!('v') => Some(DashboardAction::RemoteBrowse),
                    key!('u') => Some(DashboardAction::RemoteConnect),
                    key!('i') => Some(DashboardAction::RemoteConnections),
                    key!('f') => Some(DashboardAction::FindFile),
                    key!('r') => Some(DashboardAction::FileExplorer),
                    key!('g') => Some(DashboardAction::LiveGrep),
                    key!('o') => Some(DashboardAction::FindNotes),
                    key!('a') => Some(DashboardAction::NewNote),
                    key!('d') => Some(DashboardAction::DailyNote),
                    key!('e') => Some(DashboardAction::Agenda),
                    key!('p') => Some(DashboardAction::OpenProject),
                    key!('b') => Some(DashboardAction::AddProject),
                    key!('x') => Some(DashboardAction::ForgetProject),
                    key!('c') => Some(DashboardAction::ClearProject),
                    key!('s') => Some(DashboardAction::ProjectStatus),
                    key!('?') => Some(DashboardAction::Help),
                    key!('n') => Some(DashboardAction::NewFile),
                    key!('q') => Some(DashboardAction::Quit),
                    key!('t') => Some(DashboardAction::Terminal),
                    key!('G') => Some(DashboardAction::LazyGit),
                    key!('D') => Some(DashboardAction::LazyDocker),
                    _ => self.selected_action(),
                };
                if !action.is_some_and(|action| self.actions.contains(&action)) {
                    return EventResult::Consumed(None);
                }
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
                    if matches!(action, Some(DashboardAction::Terminal | DashboardAction::LazyGit | DashboardAction::LazyDocker)) {
                        // Terminals replace the startup dashboard; project controls do not.
                        compositor.remove_type::<Dashboard>();
                    }
                    let command = match action {
                        Some(DashboardAction::RemoteDisconnect) => {
                            #[cfg(target_os = "linux")]
                            {
                                compositor.push(Box::new(crate::ui::popup::Popup::new(
                                    "ssh-disconnect-confirm", DisconnectConfirmation { disconnect: false },
                                ).centered().with_border()));
                            }
                            #[cfg(not(target_os = "linux"))]
                            cx.editor.set_error("Remote SSH currently requires Linux");
                            return;
                        }
                        Some(DashboardAction::RemoteSavedHosts) => {
                            #[cfg(target_os = "linux")]
                            {
                                match commands::ssh_saved_hosts() {
                                    Ok(hosts) if !hosts.is_empty() => compositor.push(Box::new(crate::ui::popup::Popup::new(
                                        "ssh-saved-hosts", SavedHosts { hosts, selected: 0 },
                                    ).centered().with_border())),
                                    Ok(_) => ssh_input_error(cx, "No saved hosts yet. Connect to a host first.".into()),
                                    Err(err) => ssh_input_error(cx, format!("Could not load saved hosts: {err:#}")),
                                }
                            }
                            #[cfg(not(target_os = "linux"))]
                            cx.editor.set_error("Remote SSH currently requires Linux");
                            return;
                        }
                        Some(DashboardAction::RemoteDirectory) => {
                            #[cfg(target_os = "linux")]
                            {
                                use crate::ui::prompt::{Prompt, PromptEvent};
                                let prompt = Prompt::new("Remote directory: ".into(), None,
                                    |_, _| Vec::new(),
                                    |cx, input, event| {
                                        if event == PromptEvent::Validate {
                                            commands::ssh_change_directory(cx, input.to_string());
                                        }
                                    });
                                compositor.push(Box::new(prompt));
                            }
                            #[cfg(not(target_os = "linux"))]
                            cx.editor.set_error("Remote SSH currently requires Linux");
                            return;
                        }
                        Some(DashboardAction::RemoteBrowse) => {
                            #[cfg(target_os = "linux")]
                            commands::ssh_browse(cx);
                            #[cfg(not(target_os = "linux"))]
                            cx.editor.set_error("Remote SSH currently requires Linux");
                            return;
                        }
                        Some(DashboardAction::RemoteConnect) => {
                            #[cfg(target_os = "linux")]
                            {
                                use crate::ui::prompt::{Prompt, PromptEvent};
                                let prompt = Prompt::new("SSH host and absolute path: ".into(), None,
                                    |_, _| Vec::new(),
                                    |cx, input, event| {
                                        if event != PromptEvent::Validate { return; }
                                        let Some((host, path)) = input.trim_start().split_once(char::is_whitespace) else {
                                            ssh_input_error(cx, "Enter user@host /absolute/path (or an SSH alias followed by the path)".into());
                                            return;
                                        };
                                        let location = crate::remote::registry::RemoteLocation {
                                            destination: host.into(),
                                            remote_path: path.trim_start().into(),
                                        };
                                        if let Err(err) = commands::ssh_connect_location(cx, location) {
                                            ssh_input_error(cx, format!("{err:#}"));
                                        }
                                    });
                                compositor.push(Box::new(prompt));
                            }
                            #[cfg(not(target_os = "linux"))]
                            cx.editor.set_error("Remote SSH currently requires Linux");
                            return;
                        }
                        Some(DashboardAction::RemoteConnections) => {
                            #[cfg(target_os = "linux")]
                            {
                                if let Err(err) = commands::ssh_connections(cx, Default::default(), crate::ui::prompt::PromptEvent::Validate) {
                                    cx.editor.set_error(err.to_string());
                                }
                            }
                            #[cfg(not(target_os = "linux"))]
                            cx.editor.set_error("Remote SSH currently requires Linux");
                            return;
                        }
                        Some(DashboardAction::FindFile) => MappableCommand::file_picker,
                        Some(DashboardAction::LiveGrep) => MappableCommand::global_search,
                        Some(DashboardAction::FindNotes) => MappableCommand::primer_notes_find,
                        Some(DashboardAction::NewNote) => MappableCommand::primer_notes_new,
                        Some(DashboardAction::DailyNote) => MappableCommand::primer_notes_daily,
                        Some(DashboardAction::Agenda) => MappableCommand::primer_agenda,
                        Some(DashboardAction::FileExplorer) => MappableCommand::file_explorer,
                        Some(DashboardAction::OpenProject) => {
                            MappableCommand::primer_project_picker
                        }
                        Some(DashboardAction::AddProject) => MappableCommand::primer_project_register,
                        Some(DashboardAction::ForgetProject) => MappableCommand::primer_project_forget,
                        Some(DashboardAction::ClearProject) => MappableCommand::primer_project_clear,
                        Some(DashboardAction::ProjectStatus) => MappableCommand::primer_project_status,
                        Some(DashboardAction::Help) => MappableCommand::command_palette,
                        Some(DashboardAction::NewFile) => return,
                        Some(DashboardAction::LazyGit) => MappableCommand::primer_terminal_lazygit,
                        Some(DashboardAction::LazyDocker) => MappableCommand::primer_terminal_lazydocker,
                        Some(DashboardAction::Terminal) => MappableCommand::Typable {
                            name: "terminal".to_owned(),
                            args: String::new(),
                            doc: "Open terminal".to_owned(),
                        },
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
        assert_eq!(dashboard.actions.len(), 24);
    }

    #[test]
    fn vertical_navigation_stays_in_column_and_wraps() {
        let mut dashboard = Dashboard::new();
        dashboard.next();
        assert_eq!(
            dashboard.selected_action(),
            Some(DashboardAction::FileExplorer)
        );
        dashboard.next();
        assert_eq!(dashboard.selected_action(), Some(DashboardAction::NewFile));
        dashboard.next();
        assert_eq!(dashboard.selected_action(), Some(DashboardAction::LiveGrep));
        dashboard.next();
        assert_eq!(dashboard.selected_action(), Some(DashboardAction::NewNote));
        dashboard.next();
        assert_eq!(
            dashboard.selected_action(),
            Some(DashboardAction::DailyNote)
        );
        dashboard.next();
        assert_eq!(
            dashboard.selected_action(),
            Some(DashboardAction::FindNotes)
        );
        dashboard.next();
        assert_eq!(dashboard.selected_action(), Some(DashboardAction::Agenda));
        dashboard.next();
        assert_eq!(dashboard.selected_action(), Some(DashboardAction::Help));
        for action in [DashboardAction::Terminal, DashboardAction::LazyGit, DashboardAction::LazyDocker, DashboardAction::FindFile] {
            dashboard.next();
            assert_eq!(dashboard.selected_action(), Some(action));
        }
        dashboard.previous();
        assert_eq!(dashboard.selected_action(), Some(DashboardAction::LazyDocker));
    }

    #[test]
    fn horizontal_navigation_switches_columns() {
        let mut dashboard = Dashboard::new();
        dashboard.right();
        assert_eq!(
            dashboard.selected_action(),
            Some(DashboardAction::OpenProject)
        );
        dashboard.next();
        assert_eq!(dashboard.selected_action(), Some(DashboardAction::AddProject));
        dashboard.left();
        assert_eq!(dashboard.selected_action(), Some(DashboardAction::FindFile));
        dashboard.right();
        assert_eq!(dashboard.selected_action(), Some(DashboardAction::OpenProject));
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

#[cfg(target_os = "linux")]
fn ssh_input_error(cx: &mut Context, message: String) {
    cx.jobs.callback(async move {
        Ok(crate::job::Callback::EditorCompositor(Box::new(move |_, compositor| {
            let content = crate::ui::Text::new(format!("Invalid SSH connection\n\n{message}\n\nExample: user@host /home/user/Coding\n\nEsc to close"));
            compositor.push(Box::new(crate::ui::popup::Popup::new("ssh-input-error", content).centered().with_border()));
        })))
    });
}

#[cfg(target_os = "linux")]
pub(crate) fn authentication_dialog(compositor: &mut crate::compositor::Compositor, location: crate::remote::registry::RemoteLocation, url: String) {
    compositor.push(Box::new(crate::ui::popup::Popup::new("ssh-authentication", AuthenticationDialog { location, url, selected: 0, opened: false, cancelled: Default::default() }).centered().with_border()));
}

#[cfg(target_os = "linux")]
struct AuthenticationDialog {
    location: crate::remote::registry::RemoteLocation,
    url: String,
    selected: usize,
    opened: bool,
    cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

#[cfg(target_os = "linux")]
impl Component for AuthenticationDialog {
    fn required_size(&mut self, viewport: (u16, u16)) -> Option<(u16, u16)> {
        Some((64.min(viewport.0), 9.min(viewport.1)))
    }
    fn render(&mut self, area: Rect, surface: &mut Surface, cx: &mut Context) {
        let action = if self.opened { "Waiting for browser authentication…" } else { "Authenticate in browser" };
        render_ssh_dialog(area, surface, cx, "Tailscale authentication required", &["Cancel".into(), action.into()], Some(self.selected), "Enter select  ·  Esc cancel");
    }
    fn handle_event(&mut self, event: &Event, cx: &mut Context) -> EventResult {
        match event {
            Event::Key(event) => match *event {
                key!('j') | key!('k') | key!(Up) | key!(Down) | key!(Tab) => {
                    self.selected = 1 - self.selected;
                    EventResult::Consumed(None)
                }
                key!(Enter) if self.selected == 1 && !self.opened => {
                    match std::process::Command::new("xdg-open").arg(&self.url).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn() {
                        Ok(mut child) => {
                            std::thread::spawn(move || { let _ = child.wait(); });
                            self.opened = true;
                            self.selected = 0;
                            let cancelled = self.cancelled.clone();
                            let location = self.location.clone();
                            cx.jobs.spawn(async move {
                                use std::sync::atomic::Ordering;
                                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(180);
                                while std::time::Instant::now() < deadline && !cancelled.load(Ordering::Acquire) {
                                    let host = location.destination.clone();
                                    let ready = tokio::task::spawn_blocking(move || crate::remote::tailscale_preflight(&host).and_then(|_| crate::remote::ssh_auth_preflight(&host))).await;
                                    if matches!(&ready, Ok(Ok(()))) {
                                        if cancelled.swap(true, Ordering::AcqRel) { return Ok(()); }
                                        crate::job::dispatch(|_, compositor| { compositor.remove("ssh-authentication"); }).await;
                                        let runtime = std::env::var_os("XDG_RUNTIME_DIR").map(std::path::PathBuf::from).ok_or_else(|| anyhow::anyhow!("XDG_RUNTIME_DIR is not set"))?;
                                        let callback = commands::typed::ssh_connection_job(location, runtime).await?;
                                        crate::job::dispatch_callback(callback).await;
                                        return Ok(());
                                    }
                                    let diagnostic = match ready {
                                        Ok(Err(err)) => format!("{err:#}"),
                                        Err(err) => err.to_string(),
                                        Ok(Ok(())) => unreachable!(),
                                    };
                                    if crate::remote::authentication_url(&diagnostic).is_none() {
                                        if !cancelled.swap(true, Ordering::AcqRel) {
                                            crate::job::dispatch(move |_, compositor| {
                                                compositor.remove("ssh-authentication");
                                                crate::ui::notification::show_operation(compositor, Some("ssh-connect"), crate::ui::notification::Severity::Error, "SSH connection failed", diagnostic, false);
                                            }).await;
                                        }
                                        return Ok(());
                                    }
                                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                                }
                                if !cancelled.swap(true, Ordering::AcqRel) {
                                    crate::job::dispatch(|_, compositor| {
                                        compositor.remove("ssh-authentication");
                                        crate::ui::notification::show_operation(compositor, Some("ssh-connect"), crate::ui::notification::Severity::Warning, "SSH authentication timed out", "Reconnect to try again", false);
                                    }).await;
                                }
                                Ok(())
                            });
                        }
                        Err(err) => cx.editor.set_error(format!("Unable to open browser: {err}")),
                    }
                    EventResult::Consumed(None)
                }
                key!(Enter) | key!(Esc) => {
                    if self.opened && *event == key!(Enter) && self.selected == 1 { return EventResult::Consumed(None); }
                    self.cancelled.store(true, std::sync::atomic::Ordering::Release);
                    EventResult::Consumed(Some(Box::new(|compositor, _| { compositor.pop(); })))
                }
                _ => EventResult::Consumed(None),
            },
            Event::Mouse(_) => EventResult::Consumed(None),
            _ => EventResult::Ignored(None),
        }
    }
}

#[cfg(target_os = "linux")]
struct DisconnectConfirmation {
    disconnect: bool,
}

#[cfg(target_os = "linux")]
impl Component for DisconnectConfirmation {
    fn required_size(&mut self, viewport: (u16, u16)) -> Option<(u16, u16)> {
        Some((48.min(viewport.0), 8.min(viewport.1)))
    }

    fn render(&mut self, area: Rect, surface: &mut Surface, cx: &mut Context) {
        render_ssh_dialog(area, surface, cx, "Disconnect SSH?", &["Cancel".into(), "Disconnect".into()], Some(usize::from(self.disconnect)), "Enter select  ·  Esc cancel");
    }

    fn handle_event(&mut self, event: &Event, cx: &mut Context) -> EventResult {
        match event {
            Event::Key(event) => match *event {
                key!('j') | key!('k') | key!(Up) | key!(Down) | key!(Tab) => {
                    self.disconnect = !self.disconnect;
                    EventResult::Consumed(None)
                }
                key!(Enter) => {
                    if self.disconnect { commands::ssh_disconnect_dashboard(cx); }
                    EventResult::Consumed(Some(Box::new(|compositor, _| { compositor.pop(); })))
                }
                key!(Esc) => EventResult::Consumed(Some(Box::new(|compositor, _| { compositor.pop(); }))),
                _ => EventResult::Consumed(None),
            },
            Event::Mouse(_) => EventResult::Consumed(None),
            _ => EventResult::Ignored(None),
        }
    }
}

#[cfg(target_os = "linux")]
struct SavedHosts {
    hosts: Vec<crate::remote::registry::RemoteLocation>,
    selected: usize,
}

#[cfg(target_os = "linux")]
impl Component for SavedHosts {
    fn required_size(&mut self, viewport: (u16, u16)) -> Option<(u16, u16)> {
        Some(((self.hosts.iter().map(|h| h.destination.chars().count() + h.remote_path.chars().count() + 9).max().unwrap_or(48).clamp(48, 100) as u16).min(viewport.0), (self.hosts.len().min(15) as u16 + 6).min(viewport.1)))
    }

    fn render(&mut self, area: Rect, surface: &mut Surface, cx: &mut Context) {
        let labels: Vec<_> = self.hosts.iter().map(|host| format!("{}:{}", host.destination, host.remote_path)).collect();
        render_ssh_dialog(area, surface, cx, "Saved SSH Hosts", &labels, Some(self.selected), "j/k navigate  ·  Enter connect  ·  Esc cancel");
    }

    fn handle_event(&mut self, event: &Event, cx: &mut Context) -> EventResult {
        match event {
            Event::Key(event) => match *event {
                key!('j') | key!(Down) => { self.selected = (self.selected + 1) % self.hosts.len(); EventResult::Consumed(None) }
                key!('k') | key!(Up) => { self.selected = (self.selected + self.hosts.len() - 1) % self.hosts.len(); EventResult::Consumed(None) }
                key!(Enter) => {
                    if let Err(err) = commands::ssh_connect_location(cx, self.hosts[self.selected].clone()) {
                        ssh_input_error(cx, format!("{err:#}"));
                    }
                    EventResult::Consumed(Some(Box::new(|compositor, _| { compositor.pop(); })))
                }
                key!(Esc) => EventResult::Consumed(Some(Box::new(|compositor, _| { compositor.pop(); }))),
                _ => EventResult::Consumed(None),
            },
            Event::Mouse(_) => EventResult::Consumed(None),
            _ => EventResult::Ignored(None),
        }
    }
}

#[cfg(target_os = "linux")]
fn render_ssh_dialog(area: Rect, surface: &mut Surface, cx: &mut Context, title: &str, rows: &[String], selection: Option<usize>, footer: &str) {
    if area.width < 6 || area.height < 6 { return; }
    let theme = &cx.editor.theme;
    let normal = theme.get("ui.text");
    let heading = theme.try_get("ui.window").unwrap_or(normal);
    let muted = theme.try_get("ui.text.inactive").unwrap_or(normal);
    let selected = theme.get("ui.menu.selected");
    let x = area.x + 2;
    let width = (area.width - 4) as usize;
    surface.set_stringn(x, area.y + 1, title, width, heading);
    let visible = (area.height - 6) as usize;
    let offset = selection.unwrap_or(0).saturating_sub(visible.saturating_sub(1));
    for (i, label) in rows.iter().enumerate().skip(offset).take(visible) {
        let active = selection == Some(i);
        let line = format!("{} {}", if active { "›" } else { " " }, label);
        let padded = format!("{line:<width$}");
        surface.set_stringn(x, area.y + 3 + (i - offset) as u16, &padded, width, if active { selected } else { normal });
    }
    surface.set_stringn(x, area.bottom() - 2, footer, width, muted);
}

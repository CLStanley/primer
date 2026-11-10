//! Terminal session identity and focus state.
//!
//! This module intentionally does not represent terminal output as a text
//! document. A later PTY-backed session manager will own process I/O and
//! terminal screen state; editor tabs will hold `TerminalSessionId` handles.

use std::collections::HashMap;
use std::path::PathBuf;

/// Stable identity for a terminal session, independent of the active view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalSessionId(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalInputMode {
    /// Key events are interpreted by Primer (tabs, splits, navigation).
    Normal,
    /// Key events are forwarded to the running PTY application.
    Terminal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalSession {
    pub id: TerminalSessionId,
    pub title: String,
    pub working_directory: PathBuf,
    pub input_mode: TerminalInputMode,
}

/// Owns session metadata independently of the focused editor view.
///
/// Process handles and VT screen state will be added when the PTY backend is
/// implemented. This registry alone does not launch or render a terminal.
#[derive(Default)]
pub struct TerminalSessions {
    next_id: u64,
    sessions: HashMap<TerminalSessionId, TerminalSession>,
}

impl TerminalSessions {
    pub fn create(&mut self, title: impl Into<String>, working_directory: PathBuf) -> TerminalSessionId {
        let id = TerminalSessionId(self.next_id);
        self.next_id = self.next_id.checked_add(1).expect("terminal session IDs exhausted");
        self.sessions.insert(id, TerminalSession {
            id,
            title: title.into(),
            working_directory,
            input_mode: TerminalInputMode::Terminal,
        });
        id
    }

    pub fn get(&self, id: TerminalSessionId) -> Option<&TerminalSession> {
        self.sessions.get(&id)
    }

    pub fn set_mode(&mut self, id: TerminalSessionId, mode: TerminalInputMode) -> bool {
        if let Some(session) = self.sessions.get_mut(&id) {
            session.input_mode = mode;
            true
        } else {
            false
        }
    }

    pub fn remove(&mut self, id: TerminalSessionId) -> Option<TerminalSession> {
        self.sessions.remove(&id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_mode_and_identity_survive_focus_changes() {
        let mut sessions = TerminalSessions::default();
        let first = sessions.create("shell", PathBuf::from("/tmp"));
        let second = sessions.create("lazygit", PathBuf::from("/tmp/repo"));
        assert_ne!(first, second);
        assert_eq!(sessions.get(first).unwrap().input_mode, TerminalInputMode::Terminal);
        assert!(sessions.set_mode(first, TerminalInputMode::Normal));
        assert_eq!(sessions.get(second).unwrap().input_mode, TerminalInputMode::Terminal);
        assert_eq!(sessions.get(first).unwrap().input_mode, TerminalInputMode::Normal);
    }

    #[test]
    fn removed_sessions_are_not_reused() {
        let mut sessions = TerminalSessions::default();
        let first = sessions.create("shell", PathBuf::from("/tmp"));
        assert!(sessions.remove(first).is_some());
        let second = sessions.create("shell", PathBuf::from("/tmp"));
        assert_ne!(first, second);
        assert!(!sessions.set_mode(first, TerminalInputMode::Terminal));
    }
}

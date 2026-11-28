//! Versioned workspace metadata, independent of live editor state.
use std::{fs, io, path::{Path, PathBuf}};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::registry::ProjectRegistry;

const VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ProjectIdentity {
    Local { root: PathBuf },
    Remote { destination: String, root: PathBuf },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentSnapshot {
    pub path: PathBuf,
    pub cursor: usize,
    pub scroll: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceSnapshot {
    pub project: ProjectIdentity,
    pub documents: Vec<DocumentSnapshot>,
    pub active_document: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionStore {
    version: u32,
    sessions: Vec<WorkspaceSnapshot>,
}

impl Default for SessionStore {
    fn default() -> Self {
        Self { version: VERSION, sessions: Vec::new() }
    }
}

impl SessionStore {
    pub fn default_path() -> Result<PathBuf> {
        Ok(ProjectRegistry::default_path()?.with_file_name("sessions.json"))
    }

    pub fn read(path: &Path) -> Result<Self> {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(error) => return Err(error.into()),
        };
        let store: Self = serde_json::from_slice(&bytes).context("Invalid project session store")?;
        anyhow::ensure!(store.version == VERSION, "Unsupported project session version: {}", store.version);
        Ok(store)
    }

    pub fn get(&self, project: &ProjectIdentity) -> Option<&WorkspaceSnapshot> {
        self.sessions.iter().find(|session| &session.project == project)
    }

    pub fn insert(&mut self, snapshot: WorkspaceSnapshot) {
        self.clear(&snapshot.project);
        self.sessions.push(snapshot);
    }

    pub fn clear(&mut self, project: &ProjectIdentity) {
        self.sessions.retain(|session| &session.project != project);
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let parent = path.parent().context("Session store has no parent directory")?;
        fs::create_dir_all(parent)?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        serde_json::to_writer_pretty(temporary.as_file_mut(), self)?;
        temporary.as_file().sync_all()?;
        temporary.persist(path).context("Replacing project session store")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_clear_are_identity_scoped() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("sessions.json");
        let mut store = SessionStore::read(&path).unwrap();
        let local = ProjectIdentity::Local { root: PathBuf::from("/code") };
        let remote = ProjectIdentity::Remote { destination: "user@host".into(), root: PathBuf::from("/code") };
        for project in [local.clone(), remote.clone()] {
            store.insert(WorkspaceSnapshot {
                project, documents: vec![DocumentSnapshot { path: PathBuf::from("/code/main.rs"), cursor: 12, scroll: 3 }],
                active_document: Some(0),
            });
        }
        store.save(&path).unwrap();
        let mut restored = SessionStore::read(&path).unwrap();
        assert_eq!(restored.get(&local), store.get(&local));
        restored.clear(&local);
        restored.save(&path).unwrap();
        let restored = SessionStore::read(&path).unwrap();
        assert!(restored.get(&local).is_none());
        assert!(restored.get(&remote).is_some());
    }

    #[test]
    fn invalid_or_unknown_version_is_not_treated_as_empty() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("sessions.json");
        for data in ["not json", r#"{"version":99,"sessions":[]}"#] {
            fs::write(&path, data).unwrap();
            assert!(SessionStore::read(&path).is_err());
            assert_eq!(fs::read_to_string(&path).unwrap(), data);
        }
    }
}

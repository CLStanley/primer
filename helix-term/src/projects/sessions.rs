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
    #[serde(default)]
    pub ranges: Vec<(usize, usize)>,
    #[serde(default)]
    pub primary: usize,
    #[serde(default)]
    pub horizontal_offset: usize,
    #[serde(default)]
    pub vertical_offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum PaneLayout {
    Pane { document: DocumentSnapshot, focused: bool },
    Group { horizontal: bool, children: Vec<PaneLayout> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceSnapshot {
    pub project: ProjectIdentity,
    pub documents: Vec<DocumentSnapshot>,
    pub active_document: Option<usize>,
    #[serde(default)]
    pub layout: Option<PaneLayout>,
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

/// Capture file-backed buffers in the project, without modifying the editor.
/// For remote projects, mounted_root is the local mount of the project root.
pub fn capture(
    editor: &helix_view::Editor,
    project: ProjectIdentity,
    mounted_root: &Path,
) -> Result<WorkspaceSnapshot> {
    let root = match &project {
        ProjectIdentity::Local { root } | ProjectIdentity::Remote { root, .. } => root,
    };
    anyhow::ensure!(root.is_absolute() && mounted_root.is_absolute(), "Project roots must be absolute");
    let focused = editor.tree.views().find(|(_, focused)| *focused).map(|(view, _)| view);
    let mut documents = Vec::new();
    let mut active_document = None;
    for doc in editor.documents() {
        let Some(path) = doc.path() else { continue };
        let Ok(relative) = path.strip_prefix(mounted_root) else { continue };
        let view = focused.filter(|view| doc.selections().contains_key(&view.id))
            .or_else(|| editor.tree.views().map(|(view, _)| view)
                .find(|view| doc.selections().contains_key(&view.id)));
        let (cursor, scroll) = view.map(|view| (
            doc.selection(view.id).primary().cursor(doc.text().slice(..)),
            doc.view_offset(view.id).anchor,
        )).unwrap_or_default();
        if focused.is_some_and(|view| view.doc == doc.id()) {
            active_document = Some(documents.len());
        }
        let (ranges, primary, horizontal_offset, vertical_offset) = view.map(|view| {
            let selection = doc.selection(view.id);
            let offset = doc.view_offset(view.id);
            (selection.ranges().iter().map(|range| (range.anchor, range.head)).collect(),
             selection.primary_index(), offset.horizontal_offset, offset.vertical_offset)
        }).unwrap_or_default();
        documents.push(DocumentSnapshot { path: root.join(relative), cursor, scroll,
            ranges, primary, horizontal_offset, vertical_offset });
    }
    fn capture_layout(
        editor: &helix_view::Editor,
        tree: helix_view::tree::SplitTree,
        root: &Path,
        mounted_root: &Path,
    ) -> Option<PaneLayout> {
        use helix_view::tree::{Layout, SplitTree};
        match tree {
            SplitTree::Pane(id) => {
                let view = editor.tree.get(id);
                let doc = editor.document(view.doc)?;
                let relative = doc.path()?.strip_prefix(mounted_root).ok()?;
                let selection = doc.selection(id);
                let offset = doc.view_offset(id);
                Some(PaneLayout::Pane {
                    document: DocumentSnapshot {
                        path: root.join(relative),
                        cursor: selection.primary().cursor(doc.text().slice(..)),
                        scroll: offset.anchor,
                        ranges: selection.ranges().iter().map(|range| (range.anchor, range.head)).collect(),
                        primary: selection.primary_index(),
                        horizontal_offset: offset.horizontal_offset,
                        vertical_offset: offset.vertical_offset,
                    },
                    focused: editor.tree.focus == id,
                })
            }
            SplitTree::Group { layout, children } => {
                let mut children: Vec<_> = children.into_iter()
                    .filter_map(|child| capture_layout(editor, child, root, mounted_root)).collect();
                match children.len() {
                    0 => None,
                    1 => children.pop(),
                    _ => Some(PaneLayout::Group { horizontal: layout == Layout::Horizontal, children }),
                }
            }
        }
    }
    let layout = capture_layout(editor, editor.tree.split_tree(), root, mounted_root);
    Ok(WorkspaceSnapshot { project, documents, active_document, layout })
}

/// Restore file-backed buffers additively. Never close existing documents.
/// Resolve all paths before opening any buffers; reject paths outside the project.
pub fn restore(
    editor: &mut helix_view::Editor,
    snapshot: &WorkspaceSnapshot,
    mounted_root: &Path,
) -> Result<()> {
    use helix_view::{editor::Action, view::ViewPosition};
    use helix_core::{Range, Selection};
    let root = match &snapshot.project {
        ProjectIdentity::Local { root } | ProjectIdentity::Remote { root, .. } => root,
    };
    anyhow::ensure!(root.is_absolute() && mounted_root.is_absolute(), "Project roots must be absolute");
    anyhow::ensure!(snapshot.active_document.is_none_or(|index| index < snapshot.documents.len()), "Invalid active document index");
    let canonical_root = fs::canonicalize(mounted_root)?;
    let paths = snapshot.documents.iter().map(|doc| -> Result<PathBuf> {
        let relative = doc.path.strip_prefix(root).context("Session document outside project")?;
        anyhow::ensure!(relative.components().all(|part| matches!(part, std::path::Component::Normal(_))), "Invalid session document path");
        let path = fs::canonicalize(canonical_root.join(relative))?;
        anyhow::ensure!(path.starts_with(&canonical_root) && path.is_file(), "Session document outside project or not a file");
        Ok(path)
    }).collect::<Result<Vec<_>>>()?;
    let mut active_id = None;
    for (index, (path, saved)) in paths.iter().zip(&snapshot.documents).enumerate() {
        let action = if editor.tree.is_empty() { Action::VerticalSplit } else { Action::Load };
        let id = editor.open(path, action)?;
        let view_id = editor.tree.focus;
        if let Some(doc) = editor.document_mut(id) {
            let len = doc.text().len_chars();
            let selection = if saved.ranges.is_empty() {
                Selection::point(saved.cursor.min(len))
            } else {
                Selection::new(saved.ranges.iter().map(|&(anchor, head)| Range::new(anchor.min(len), head.min(len))).collect(),
                    saved.primary.min(saved.ranges.len() - 1))
            };
            doc.set_selection(view_id, selection);
            doc.set_view_offset(view_id, ViewPosition { anchor: saved.scroll.min(len),
                horizontal_offset: saved.horizontal_offset, vertical_offset: saved.vertical_offset });
        }
        if snapshot.active_document == Some(index) {
            active_id = Some(id);
        }
    }
    if let Some(id) = active_id {
        editor.switch(id, Action::Replace);
    }
    Ok(())
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
                project, documents: vec![DocumentSnapshot { path: PathBuf::from("/code/main.rs"), cursor: 12, scroll: 3, ranges: vec![(12, 8), (20, 22)], primary: 1, horizontal_offset: 4, vertical_offset: 2 }],
                active_document: Some(0),
                layout: None,
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

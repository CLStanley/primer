//! Git-root discovery and manually remembered directory projects.
//! No project marker or per-repository configuration is required.
pub mod registry;
pub mod sessions;

use std::path::{Path, PathBuf};
use anyhow::{Context, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    pub root: PathBuf,
    pub name: String,
}

pub fn from_directory(path: &Path) -> Result<Project> {
    let root = std::fs::canonicalize(path)
        .with_context(|| format!("resolving project directory {}", path.display()))?;
    anyhow::ensure!(root.is_dir(), "not a directory: {}", root.display());
    let name = root.file_name().unwrap_or(root.as_os_str()).to_string_lossy().into_owned();
    Ok(Project { root, name })
}

/// Discover the containing Git worktree. Do not treat package manifests as projects.
pub fn discover(path: &Path) -> Result<Option<Project>> {
    let start = if path.is_dir() { path } else { path.parent().unwrap_or(path) };
    let mut current = std::fs::canonicalize(start)
        .with_context(|| format!("resolving {}", start.display()))?;
    loop {
        if current.join(".git").exists() {
            return from_directory(&current).map(Some);
        }
        if !current.pop() { break; }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn directory_name_is_project_name() {
        let root = std::env::temp_dir();
        let project = from_directory(&root).unwrap();
        assert_eq!(project.root, std::fs::canonicalize(&root).unwrap());
        assert!(!project.name.is_empty());
    }
}

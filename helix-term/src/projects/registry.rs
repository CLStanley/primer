//! Persistent local index of discovered Git projects and manually added directories.
//! No files are created in project directories.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::{from_directory, Project};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ProjectRegistry {
    #[serde(default)]
    pub roots: Vec<PathBuf>,
    #[serde(default)]
    pub remote_roots: Vec<RemoteProject>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteProject {
    pub destination: String,
    pub path: PathBuf,
}

impl ProjectRegistry {
    /// Remember a stable remote identity, never an SSHFS mount path.
    pub fn register_remote(&mut self, destination: &str, path: &Path) -> Result<()> {
        anyhow::ensure!(!destination.is_empty() && path.is_absolute(), "Invalid remote project identity");
        let project = RemoteProject { destination: destination.to_owned(), path: path.to_owned() };
        self.remote_roots.retain(|existing| existing != &project);
        self.remote_roots.insert(0, project);
        Ok(())
    }

    pub fn forget_remote(&mut self, destination: &str, path: &Path) {
        self.remote_roots.retain(|project| project.destination != destination || project.path != path);
    }

    pub fn remote_projects<'a>(&'a self, destination: &'a str) -> impl Iterator<Item = &'a RemoteProject> + 'a {
        self.remote_roots.iter().filter(move |project| project.destination == destination)
    }

    /// XDG_DATA_HOME/primer/projects.toml, falling back to ~/.local/share.
    pub fn default_path() -> Result<PathBuf> {
        let data_home = std::env::var_os("XDG_DATA_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
            .context("cannot locate project registry: set XDG_DATA_HOME or HOME")?;
        Ok(data_home.join("primer/projects.toml"))
    }

    pub fn read(path: &Path) -> Result<Self> {
        let source = match fs::read_to_string(path) {
            Ok(source) => source,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(error) => return Err(error).with_context(|| format!("reading {}", path.display())),
        };
        toml::from_str(&source).with_context(|| format!("invalid registry {}", path.display()))
    }

    /// Remember an existing directory, most recently visited first.
    pub fn register(&mut self, root: &Path) -> Result<Project> {
        let project = from_directory(root)?;
        self.roots.retain(|existing| existing != &project.root);
        self.roots.insert(0, project.root.clone());
        Ok(project)
    }

    pub fn forget(&mut self, root: &Path) {
        let canonical = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
        self.roots.retain(|existing| existing != &canonical);
    }

    /// Missing directories are skipped; no marker is required.
    pub fn projects(&self) -> Result<Vec<Project>> {
        let mut seen = std::collections::HashSet::new();
        Ok(self.roots.iter()
            .filter_map(|root| from_directory(root).ok())
            .filter(|project| seen.insert(project.root.clone()))
            .collect())
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let parent = path.parent().context("registry path has no parent directory")?;
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        let serialized = toml::to_string_pretty(self).context("serializing project registry")?;
        // Create a sibling temporary file, then rename to avoid a truncated registry.
        let temporary = path.with_extension(format!("toml.tmp-{}", std::process::id()));
        let result = (|| -> Result<()> {
            fs::write(&temporary, serialized).with_context(|| format!("writing {}", temporary.display()))?;
            fs::rename(&temporary, path).with_context(|| format!("replacing {}", path.display()))?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn remote_projects_are_host_scoped_and_persisted() {
        let mut registry = ProjectRegistry::default();
        let path = Path::new("/home/user/Coding");
        registry.register_remote("host-a", path).unwrap();
        registry.register_remote("host-b", path).unwrap();
        registry.register_remote("host-a", path).unwrap();
        assert_eq!(registry.remote_projects("host-a").count(), 1);
        assert!(registry.roots.is_empty());
        let mut restored: ProjectRegistry = toml::from_str(&toml::to_string(&registry).unwrap()).unwrap();
        restored.forget_remote("host-a", path);
        assert_eq!(restored.remote_projects("host-a").count(), 0);
        assert_eq!(restored.remote_projects("host-b").count(), 1);
    }

    #[test]
    fn register_without_marker_and_forget() {
        let root = std::env::temp_dir();
        let mut registry = ProjectRegistry::default();
        registry.register(&root).unwrap();
        registry.register(&root).unwrap();
        assert_eq!(registry.roots.len(), 1);
        registry.forget(&root);
        assert!(registry.roots.is_empty());
    }
}

//! Persistent local index of explicitly initialized Primer projects.
//! This index is a convenience cache; the `.primer.toml` marker is authoritative.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::{load, Project};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ProjectRegistry {
    #[serde(default)]
    pub roots: Vec<PathBuf>,
}

impl ProjectRegistry {
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

    /// Register only directories with a valid explicit project marker.
    /// Most recently registered projects appear first.
    pub fn register(&mut self, root: &Path) -> Result<Project> {
        let project = load(root)?.with_context(|| {
            format!("{} is not a Primer project; initialize it first", root.display())
        })?;
        self.roots.retain(|existing| existing != &project.root);
        self.roots.insert(0, project.root.clone());
        Ok(project)
    }

    /// Forgetting a project does not delete its marker or files.
    pub fn forget(&mut self, root: &Path) {
        let canonical = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
        self.roots.retain(|existing| existing != &canonical);
    }

    /// Return valid projects in recent-first order, skipping missing markers.
    /// Invalid marker contents are reported rather than silently reinterpreted.
    pub fn projects(&self) -> Result<Vec<Project>> {
        self.roots.iter().filter_map(|root| match load(root) {
            Ok(Some(project)) => Some(Ok(project)),
            Ok(None) => None,
            Err(error) => Some(Err(error)),
        }).collect()
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
    use crate::projects::initialize;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn register_requires_marker_and_forget_preserves_files() {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("primer-registry-{}-{stamp}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let mut registry = ProjectRegistry::default();
        assert!(registry.register(&root).is_err());
        initialize(&root, "Example").unwrap();
        registry.register(&root).unwrap();
        registry.register(&root).unwrap();
        assert_eq!(registry.roots.len(), 1);
        let file = root.join("projects.toml");
        registry.save(&file).unwrap();
        let mut restored = ProjectRegistry::read(&file).unwrap();
        assert_eq!(restored.projects().unwrap()[0].settings.project.name, "Example");
        restored.forget(&root);
        assert!(restored.roots.is_empty());
        assert!(root.join(super::super::MARKER).exists());
        fs::remove_dir_all(root).unwrap();
    }
}

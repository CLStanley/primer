//! Explicit Primer project identity and initialization.
//!
//! Projects are never inferred from Git, language servers, or directory layout.
//! Only a valid `.primer.toml` at the root makes a directory a project.

pub mod registry;

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

pub const MARKER: &str = ".primer.toml";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectSettings {
    pub project: ProjectMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectMetadata {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    pub root: PathBuf,
    pub settings: ProjectSettings,
}

/// Read an explicitly initialized project. Returns None for unmarked directories.
/// A present but invalid marker is an error, not an invitation to infer a project.
pub fn load(root: &Path) -> Result<Option<Project>> {
    let marker = root.join(MARKER);
    let source = match fs::read_to_string(&marker) {
        Ok(source) => source,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).with_context(|| format!("reading {}", marker.display())),
    };
    let settings: ProjectSettings = toml::from_str(&source)
        .with_context(|| format!("invalid project configuration: {}", marker.display()))?;
    validate_name(&settings.project.name)?;
    let root = fs::canonicalize(root)
        .with_context(|| format!("resolving project root {}", root.display()))?;
    Ok(Some(Project { root, settings }))
}

/// Explicitly initialize an existing directory. Never replace an existing marker.
pub fn initialize(root: &Path, name: &str) -> Result<Project> {
    validate_name(name)?;
    if !root.is_dir() {
        bail!("project directory does not exist: {}", root.display());
    }
    let root = fs::canonicalize(root)
        .with_context(|| format!("resolving project root {}", root.display()))?;
    let marker = root.join(MARKER);
    let settings = ProjectSettings {
        project: ProjectMetadata { name: name.to_owned() },
    };
    let serialized = toml::to_string_pretty(&settings)
        .context("serializing project configuration")?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&marker)
        .with_context(|| format!("creating {} (will not overwrite existing configuration)", marker.display()))?;
    if let Err(error) = file.write_all(serialized.as_bytes()) {
        // Do not leave a partially written marker after an unsuccessful initialization.
        drop(file);
        let _ = fs::remove_file(&marker);
        return Err(error).with_context(|| format!("writing {}", marker.display()));
    }
    Ok(Project { root, settings })
}

fn validate_name(name: &str) -> Result<()> {
    if name.trim().is_empty() {
        bail!("project name must not be empty");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_directory() -> PathBuf {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("primer-project-test-{}-{unique}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        dir
    }

    #[test]
    fn unmarked_directory_is_not_a_project() {
        let dir = test_directory();
        assert!(load(&dir).unwrap().is_none());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn initialization_round_trips_and_never_overwrites() {
        let dir = test_directory();
        let project = initialize(&dir, "Primer").unwrap();
        assert_eq!(project.settings.project.name, "Primer");
        assert_eq!(load(&dir).unwrap(), Some(project));
        assert!(initialize(&dir, "Replacement").is_err());
        assert_eq!(load(&dir).unwrap().unwrap().settings.project.name, "Primer");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rejects_empty_name_and_invalid_marker() {
        let dir = test_directory();
        assert!(initialize(&dir, "   ").is_err());
        assert!(!dir.join(MARKER).exists());
        fs::write(dir.join(MARKER), "[project]\nname = 123\n").unwrap();
        assert!(load(&dir).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
}

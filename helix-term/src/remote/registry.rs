//! Session-local mount registry. Run blocking operations off the UI thread.
//! A single mutable owner serializes mount lifecycle operations.
use super::mount::{MountState, RemoteMount};
use anyhow::{ensure, Context, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RemoteLocation {
    pub destination: String,
    pub remote_path: String,
}

#[derive(Debug, Default)]
pub struct ConnectionRegistry {
    mounts: BTreeMap<RemoteLocation, RemoteMount>,
}

impl ConnectionRegistry {
    pub fn entries(&self) -> impl Iterator<Item = (&RemoteLocation, &RemoteMount)> {
        self.mounts.iter()
    }

    pub fn get(&self, location: &RemoteLocation) -> Option<&RemoteMount> {
        self.mounts.get(location)
    }

    /// Reuse a mounted location, or retry its existing protected directory.
    /// Keep failed attempts registered so their resources remain accountable.
    pub fn connect(&mut self, runtime: &Path, location: RemoteLocation) -> Result<PathBuf> {
        // An editor instance owns at most one mounted remote environment.
        // Do not silently replace it while buffers or jobs may still use it.
        for (existing, mount) in &self.mounts {
            if existing != &location {
                ensure!(mount.state()? == MountState::Unmounted,
                    "A remote environment is already mounted ({}:{}). Disconnect it before connecting to another directory or host.",
                    existing.destination, existing.remote_path);
            }
        }
        if !self.mounts.contains_key(&location) {
            let mount = RemoteMount::prepare(runtime, &location.destination, &location.remote_path)?;
            self.mounts.insert(location.clone(), mount);
        }
        let mount = self.mounts.get(&location).context("Missing remote mount")?;
        match mount.state()? {
            MountState::Mounted => {}
            MountState::Unmounted => mount.connect()?,
            MountState::UnexpectedFilesystem => anyhow::bail!("Unexpected filesystem at remote mount"),
        }
        Ok(mount.path().to_owned())
    }

    /// Call only after the editor has checked open documents and active jobs.
    /// Keep the record on failure so the caller can retry or report it.
    pub fn disconnect(&mut self, location: &RemoteLocation) -> Result<()> {
        let mount = self.mounts.get(location).context("Unknown remote connection")?;
        mount.disconnect()?;
        mount.cleanup()?;
        self.mounts.remove(location);
        Ok(())
    }

    /// Lexical lookup deliberately avoids filesystem I/O on disconnected mounts.
    /// Paths must be absolute, normalized editor paths; this is not a security boundary.
    pub fn location_for_path(&self, path: &Path) -> Option<&RemoteLocation> {
        self.mounts.iter()
            .filter(|(_, mount)| path.starts_with(mount.path()))
            .max_by_key(|(_, mount)| mount.path().components().count())
            .map(|(location, _)| location)
    }

    pub fn mounted_path(&self, location: &RemoteLocation) -> Result<&Path> {
        let mount = self.mounts.get(location).context("Unknown remote connection")?;
        ensure!(mount.state()? == MountState::Mounted, "Remote filesystem is not mounted");
        Ok(mount.path())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_identity_includes_host_and_directory() {
        let a = RemoteLocation { destination: "nas".into(), remote_path: "/etc".into() };
        let b = RemoteLocation { destination: "debian".into(), remote_path: "/etc".into() };
        let c = RemoteLocation { destination: "nas".into(), remote_path: "/home".into() };
        assert_ne!(a, b);
        assert_ne!(a, c);
        let mut locations = std::collections::BTreeSet::new();
        locations.insert(a.clone());
        locations.insert(a);
        locations.insert(b);
        locations.insert(c);
        assert_eq!(locations.len(), 3);
    }

    #[test]
    fn empty_registry_has_no_remote_context() {
        let registry = ConnectionRegistry::default();
        let location = RemoteLocation { destination: "nas".into(), remote_path: "/".into() };
        assert!(registry.get(&location).is_none());
        assert!(registry.mounted_path(&location).is_err());
        assert!(registry.location_for_path(Path::new("/tmp/local")).is_none());
        assert_eq!(registry.entries().count(), 0);
    }
}

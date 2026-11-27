//! Linux SSHFS lifecycle. Call blocking operations from a background worker.
use super::mount_source;
use anyhow::{bail, ensure, Context, Result};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_MOUNT: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MountState {
    Mounted,
    Unmounted,
    UnexpectedFilesystem,
}

#[derive(Debug)]
pub struct RemoteMount {
    destination: String,
    remote_path: String,
    path: PathBuf,
    source: String,
}

impl RemoteMount {
    /// Allocate a fresh, non-writable underlying directory. Never reuse stale mounts.
    /// The caller must supply a private, user-owned runtime directory.
    pub fn prepare(runtime: &Path, destination: &str, remote_path: &str) -> Result<Self> {
        let source = mount_source(destination, remote_path)?;
        let runtime = runtime.canonicalize().context("Resolving mount runtime directory")?;
        ensure!(runtime.is_dir(), "Mount runtime is not a directory");
        ensure!(runtime.metadata()?.permissions().mode() & 0o077 == 0,
            "Mount runtime directory must be private");
        let path = loop {
            let id = NEXT_MOUNT.fetch_add(1, Ordering::Relaxed);
            let path = runtime.join(format!("primer-ssh-{}-{id}", std::process::id()));
            match std::fs::DirBuilder::new().mode(0o500).create(&path) {
                Ok(()) => break path,
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(err) => return Err(err).context("Creating protected mount directory"),
            }
        };
        Ok(Self { destination: destination.into(), remote_path: remote_path.into(), path, source })
    }

    pub fn path(&self) -> &Path { &self.path }
    pub fn destination(&self) -> &str { &self.destination }
    pub fn remote_path(&self) -> &str { &self.remote_path }

    /// Kernel mount presence, not a network liveness probe.
    pub fn state(&self) -> Result<MountState> {
        let info = std::fs::read_to_string("/proc/self/mountinfo")?;
        Ok(mount_state(&info, &self.path, &self.source))
    }

    /// Noninteractive initial mount. Authentication prompts must be handled
    /// separately in an interactive OpenSSH session before retrying.
    pub fn connect(&self) -> Result<()> { self.connect_with_session(None) }

    pub fn connect_with_session(&self, session: Option<&super::session::Session>) -> Result<()> {
        ensure!(self.state()? == MountState::Unmounted, "Mount directory is already mounted");
        // FUSE requires owner write access to the underlying mountpoint.
        // Restore protection on failure, but never chmod a mounted remote directory.
        std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o700))?;
        let mut command = Command::new("sshfs");
        if let Some(session) = session {
            ensure!(session.ready(), "SSH session is no longer available");
            command.arg("-o").arg(format!("ControlPath={}", session.control_path().display())).args(["-o", "ControlMaster=no"]);
        }
        let output = command
            .arg(&self.source).arg(&self.path)
            .args(["-o", "BatchMode=yes,StrictHostKeyChecking=yes,ConnectTimeout=10,ServerAliveInterval=15,ServerAliveCountMax=2"])
            .stdin(Stdio::null()).output();
        if self.state()? == MountState::Unmounted {
            std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o500))?;
        }
        let output = output.context("Launching sshfs (is it installed?)")?;
        ensure!(output.status.success(), "SSHFS failed: {}", String::from_utf8_lossy(&output.stderr).trim());
        ensure!(self.state()? == MountState::Mounted, "SSHFS did not establish the expected mount");
        Ok(())
    }

    /// Never force or lazily detach a busy filesystem. The UI must first
    /// check for open buffers, unsaved changes and active filesystem jobs.
    pub fn disconnect(&self) -> Result<()> {
        match self.state()? {
            MountState::Unmounted => {
                std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o500))?;
                return Ok(());
            },
            MountState::UnexpectedFilesystem => bail!("Refusing to unmount an unexpected filesystem"),
            MountState::Mounted => {}
        }
        let output = Command::new("fusermount3").arg("-u").arg(&self.path)
            .stdin(Stdio::null()).output().context("Launching fusermount3")?;
        ensure!(output.status.success(), "Unmount failed: {}", String::from_utf8_lossy(&output.stderr).trim());
        ensure!(self.state()? == MountState::Unmounted, "Mount is still present");
        std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o500))?;
        Ok(())
    }

    /// Explicit cleanup only; never remove a mounted directory or its contents.
    pub fn cleanup(&self) -> Result<()> {
        ensure!(self.state()? == MountState::Unmounted, "Refusing to remove a mounted directory");
        std::fs::remove_dir(&self.path).context("Removing empty mount directory")
    }
}

fn decode_mount_field(field: &str) -> Vec<u8> {
    let bytes = field.as_bytes();
    let mut result = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 3 < bytes.len()
            && bytes[i+1..i+4].iter().all(|b| matches!(*b, b'0'..=b'7')) {
            let n = (bytes[i+1]-b'0') as u16 * 64 + (bytes[i+2]-b'0') as u16 * 8 + (bytes[i+3]-b'0') as u16;
            if n <= 255 {
                result.push(n as u8);
                i += 4;
                continue;
            }
        }
        result.push(bytes[i]);
        i += 1;
    }
    result
}

fn mount_state(info: &str, path: &Path, source: &str) -> MountState {
    use std::os::unix::ffi::OsStrExt;
    let mut state = MountState::Unmounted;
    for line in info.lines() {
        let Some((left, right)) = line.split_once(" - ") else { continue };
        let Some(target) = left.split_whitespace().nth(4) else { continue };
        if decode_mount_field(target) != path.as_os_str().as_bytes() { continue; }
        let mut fields = right.split_whitespace();
        let expected = fields.next() == Some("fuse.sshfs")
            && fields.next().is_some_and(|s| decode_mount_field(s) == source.as_bytes());
        if state != MountState::Unmounted || !expected {
            return MountState::UnexpectedFilesystem;
        }
        state = MountState::Mounted;
    }
    state
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_expected_mount_and_escaped_paths() {
        let info = r"42 1 0:2 / /tmp/my\040mount rw - fuse.sshfs nas:/my\040files rw";
        let path = Path::new("/tmp/my mount");
        assert_eq!(mount_state(info, path, "nas:/my files"), MountState::Mounted);
        assert_eq!(mount_state(info, path, "other:/my files"), MountState::UnexpectedFilesystem);
        assert_eq!(mount_state(info, Path::new("/tmp/other"), "nas:/my files"), MountState::Unmounted);
    }

    #[test]
    fn rejects_unrelated_filesystems_and_stacked_mounts() {
        let info = "42 1 0:2 / /tmp/mount rw - tmpfs tmpfs rw";
        assert_eq!(mount_state(info, Path::new("/tmp/mount"), "nas:/"), MountState::UnexpectedFilesystem);
        let line = "42 1 0:2 / /tmp/mount rw - fuse.sshfs nas:/ rw";
        assert_eq!(mount_state(&format!("{line}\n{line}"), Path::new("/tmp/mount"), "nas:/"), MountState::UnexpectedFilesystem);
    }
}

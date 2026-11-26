//! Own a private OpenSSH multiplexing session without retaining credentials.
use anyhow::{ensure, Result};
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug)]
pub struct Session {
    directory: PathBuf,
    destination: String,
    child: Option<Child>,
}

impl Session {
    pub fn new(runtime: &Path, destination: &str) -> Result<Self> {
        ensure!(super::valid_destination(destination), "Invalid SSH destination");
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let directory = runtime.join(format!("primer-session-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::DirBuilder::new().mode(0o700).create(&directory)?;
        Ok(Self { directory, destination: destination.into(), child: None })
    }

    pub fn control_path(&self) -> PathBuf {
        self.directory.join("control")
    }

    /// The caller configures askpass before starting this foreground master.
    pub fn command(&self) -> Command {
        let mut command = Command::new("ssh");
        command.args(["-N", "-T", "-M", "-S"]).arg(self.control_path())
            .args(["-o", "ControlPersist=no", "-o", "StrictHostKeyChecking=yes",
                "-o", "BatchMode=no", "-o", "ClearAllForwardings=yes",
                "-o", "PermitLocalCommand=no", "-o", "ConnectTimeout=10",
                "-o", "ServerAliveInterval=15", "-o", "ServerAliveCountMax=2",
                "--", &self.destination])
            .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
        command
    }

    pub fn start(&mut self, command: &mut Command) -> Result<()> {
        ensure!(self.child.is_none(), "SSH session already started");
        self.child = Some(command.spawn()?);
        Ok(())
    }

    pub fn child_mut(&mut self) -> Option<&mut Child> {
        self.child.as_mut()
    }

    pub fn ready(&self) -> bool {
        self.control_path().exists() && Command::new("ssh")
            .args(["-S"]).arg(self.control_path())
            .args(["-O", "check", "--", &self.destination])
            .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
            .status().is_ok_and(|status| status.success())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = std::fs::remove_file(self.control_path());
        let _ = std::fs::remove_dir(&self.directory);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn session_directory_is_private_and_removed_on_drop() {
        let runtime = tempfile::tempdir().unwrap();
        let session = Session::new(runtime.path(), "user@host").unwrap();
        let directory = session.control_path().parent().unwrap().to_owned();
        assert_eq!(std::fs::metadata(&directory).unwrap().permissions().mode() & 0o777, 0o700);
        assert!(!session.ready());
        drop(session);
        assert!(!directory.exists());
        assert!(Session::new(runtime.path(), "-invalid").is_err());
    }
}

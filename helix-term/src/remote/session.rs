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
    credential_warning: Option<String>,
}

impl Session {
    pub fn new(runtime: &Path, destination: &str) -> Result<Self> {
        ensure!(super::valid_destination(destination), "Invalid SSH destination");
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let directory = runtime.join(format!("primer-session-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::DirBuilder::new().mode(0o700).create(&directory)?;
        Ok(Self { directory, destination: destination.into(), child: None, credential_warning: None })
    }

    pub fn credential_warning(&self) -> Option<&str> { self.credential_warning.as_deref() }

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

/// A single interactive OpenSSH request. A dropped sender cancels authentication.
pub struct CredentialRequest {
    pub prompt: String,
    pub repeated_password: bool,
    pub can_remember: bool,
    pub remember: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub response: tokio::sync::oneshot::Sender<Option<String>>,
}

/// Only credential requests are accepted here. Host trust remains a separate operation.
fn credential_prompt(kind: &str, prompt: &str) -> bool {
    kind.is_empty() && !prompt.is_empty() && prompt.len() <= 16384
        && !prompt.starts_with("The authenticity of host ")
        && !prompt.contains("REMOTE HOST IDENTIFICATION HAS CHANGED")
}

pub fn request_credential(socket: &Path, kind: &str, prompt: &str) -> std::io::Result<String> {
    use std::io::{Read, Write};
    use std::time::Duration;
    if !credential_prompt(kind, prompt) {
        return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "Unsupported SSH prompt"));
    }
    let mut stream = std::os::unix::net::UnixStream::connect(socket)?;
    stream.set_read_timeout(Some(Duration::from_secs(130)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    let request = format!("{kind}\0{prompt}");
    stream.write_all(&(request.len() as u32).to_be_bytes())?;
    stream.write_all(request.as_bytes())?;
    let mut accepted = [0];
    stream.read_exact(&mut accepted)?;
    if accepted[0] != 1 {
        return Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "SSH authentication cancelled"));
    }
    let mut response = String::new();
    stream.take(16385).read_to_string(&mut response)?;
    if response.len() > 16384 || response.contains(['\n', '\r', '\0']) {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid SSH response"));
    }
    Ok(response)
}

pub fn serve_credential(stream: std::os::unix::net::UnixStream, present: impl FnOnce(CredentialRequest)) -> std::io::Result<bool> {
    serve_credential_observed(stream, present, |_| {})
}

fn serve_credential_observed(mut stream: std::os::unix::net::UnixStream, present: impl FnOnce(CredentialRequest), observe: impl FnOnce(&str)) -> std::io::Result<bool> {
    use std::io::{Read, Write};
    use std::time::{Duration, Instant};
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    let mut length = [0; 4];
    stream.read_exact(&mut length)?;
    let length = u32::from_be_bytes(length) as usize;
    if length > 16385 { return Ok(false); }
    let mut bytes = vec![0; length];
    stream.read_exact(&mut bytes)?;
    let request = String::from_utf8_lossy(&bytes);
    let Some((kind, prompt)) = request.split_once('\0') else { return Ok(false); };
    if !credential_prompt(kind, prompt) { return Ok(false); }
    let (response, mut receiver) = tokio::sync::oneshot::channel();
    present(CredentialRequest { prompt: prompt.into(), repeated_password: false, can_remember: false, remember: Default::default(), response });
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        match receiver.try_recv() {
            Ok(Some(value)) if value.len() <= 16384 && !value.contains(['\n', '\r', '\0']) => {
                stream.write_all(&[1])?;
                stream.write_all(value.as_bytes())?;
                observe(&value);
                return Ok(true);
            }
            Err(tokio::sync::oneshot::error::TryRecvError::Empty) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(25));
            }
            _ => { stream.write_all(&[0])?; return Ok(false); }
        }
    }
}

#[cfg(test)]
mod credential_tests {
    use super::*;
    #[test]
    fn credential_round_trip_and_cancellation() {
        let runtime = tempfile::tempdir().unwrap();
        let socket = super::super::host_verification::VerificationSocket::new(runtime.path()).unwrap();
        for answer in [Some("example response".to_owned()), None] {
            let expected = answer.clone();
            let path = socket.path();
            let client = std::thread::spawn(move || request_credential(&path, "", "Password: "));
            let (stream, _) = socket.listener.accept().unwrap();
            assert_eq!(serve_credential(stream, |pending| { pending.response.send(answer).unwrap(); }).unwrap(), expected.is_some());
            assert_eq!(client.join().unwrap().ok(), expected);
        }
        assert!(!credential_prompt("confirm", "Continue?"));
        assert!(!credential_prompt("", "The authenticity of host example"));
    }
}

impl Session {
    /// Authenticate once, servicing sequential askpass requests until the master is ready.
    pub fn authenticate(runtime: &Path, destination: &str, mut present: impl FnMut(CredentialRequest)) -> Result<Self> {
        use std::io::Read;
        use std::time::{Duration, Instant};
        let mut session = Self::new(runtime, destination)?;
        let socket = super::host_verification::VerificationSocket::new(runtime)?;
        socket.listener.set_nonblocking(true)?;
        let mut command = session.command();
        command.env("LC_ALL", "C")
            .env("SSH_ASKPASS", std::env::current_exe()?)
            .env("SSH_ASKPASS_REQUIRE", "force")
            .env("PRIMER_CREDENTIAL_SOCKET", socket.path())
            .env_remove("PRIMER_HOST_CONFIRMATION_SOCKET");
        session.start(&mut command)?;
        let mut stderr = session.child_mut().unwrap().stderr.take().unwrap();
        let diagnostic = std::thread::spawn(move || {
            let mut output = Vec::new();
            let _ = stderr.read_to_end(&mut output);
            String::from_utf8_lossy(&output).trim().to_owned()
        });
        let result = (|| -> Result<()> {
            let mut deadline = Instant::now() + Duration::from_secs(30);
            let mut previous_password_prompt: Option<String> = None;
            let keyring = super::credentials::available();
            let mut saved = if keyring { super::credentials::retrieve(destination).ok().flatten() } else { None };
            let candidate = std::cell::RefCell::new(None::<String>);
            loop {
                if session.ready() {
                    if let Some(password) = candidate.take() {
                        if let Err(error) = super::credentials::store(destination, &password) {
                            session.credential_warning = Some(format!("Could not save SSH password: {error:#}"));
                        }
                    }
                    return Ok(());
                }
                ensure!(session.child_mut().unwrap().try_wait()?.is_none(), "SSH authentication failed");
                ensure!(Instant::now() < deadline, "SSH authentication timed out");
                match socket.listener.accept() {
                    Ok((stream, _)) => {
                        let remember = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                        let choice = remember.clone();
                        ensure!(serve_credential_observed(stream, |mut pending| {
                            let prompt = pending.prompt.trim();
                            if prompt.ends_with("'s password:") {
                                pending.repeated_password = previous_password_prompt.as_deref() == Some(prompt);
                                candidate.replace(None);
                                previous_password_prompt = Some(prompt.to_owned());
                                pending.can_remember = keyring;
                                pending.remember = choice;
                                if let Some(password) = saved.take() {
                                    let _ = pending.response.send(Some(password));
                                    return;
                                }
                            } else {
                                previous_password_prompt = None;
                            }
                            present(pending);
                        }, |value| {
                            if remember.load(Ordering::Relaxed) { candidate.replace(Some(value.to_owned())); }
                        })?, "SSH authentication cancelled");
                        deadline = Instant::now() + Duration::from_secs(30);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(50));
                    }
                    Err(error) => return Err(error.into()),
                }
            }
        })();
        match result {
            Ok(()) => Ok(session),
            Err(error) => {
                drop(session);
                let detail = diagnostic.join().unwrap_or_default();
                if detail.is_empty() { Err(error) } else { Err(anyhow::anyhow!("{error:#}: {detail}")) }
            }
        }
    }
}

//! Host confirmation data from OpenSSH's askpass interface.
//! Never treat password or keyboard-interactive prompts as host confirmation.

#[derive(Debug, PartialEq, Eq)]
pub struct HostConfirmation {
    pub fingerprint: String,
}

/// Accept only OpenSSH's confirmation prompt type and SHA256 host fingerprint.
/// The eventual response must be returned to the same waiting SSH process.
pub fn parse_confirmation(kind: &str, prompt: &str) -> Option<HostConfirmation> {
    if !kind.is_empty() && kind != "confirm"
        || !prompt.starts_with("The authenticity of host ")
        || !prompt.ends_with("Are you sure you want to continue connecting (yes/no/[fingerprint])? ")
            && !prompt.ends_with("Are you sure you want to continue connecting (yes/no/[fingerprint])?")
    {
        return None;
    }
    let mut fingerprints = prompt.lines().filter_map(|line| {
        let (_, value) = line.split_once(" key fingerprint is ")?;
        let encoded = value.strip_prefix("SHA256:")?.strip_suffix('.')?;
        if encoded.len() != 43
            || !encoded.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'+' || byte == b'/')
        {
            return None;
        }
        Some(format!("SHA256:{encoded}"))
    });
    let fingerprint = fingerprints.next()?;
    if fingerprints.next().is_some() {
        return None;
    }
    Some(HostConfirmation { fingerprint })
}

/// A decision is scoped to a single pending OpenSSH confirmation.
#[derive(Debug, PartialEq, Eq)]
pub enum HostDecision {
    Reject,
    Trust,
    Fingerprint(String),
}

impl HostConfirmation {
    /// Return the exact fingerprint even for explicit trust, so OpenSSH
    /// validates the response against the key in its current handshake.
    pub fn response(&self, decision: HostDecision) -> &'_ str {
        match decision {
            HostDecision::Trust => &self.fingerprint,
            HostDecision::Fingerprint(value) if value.trim() == self.fingerprint => &self.fingerprint,
            _ => "no",
        }
    }
}

/// One-shot response channel. Dropping the UI sender rejects the request.
pub struct PendingConfirmation {
    pub confirmation: HostConfirmation,
    pub decision: tokio::sync::oneshot::Sender<HostDecision>,
}

pub fn confirmation_channel(confirmation: HostConfirmation) -> (
    PendingConfirmation,
    tokio::sync::oneshot::Receiver<HostDecision>,
) {
    let (decision, receiver) = tokio::sync::oneshot::channel();
    (PendingConfirmation { confirmation, decision }, receiver)
}

/// Own the private socket directory for the duration of a verification attempt.
pub struct VerificationSocket {
    directory: std::path::PathBuf,
    pub listener: std::os::unix::net::UnixListener,
}

impl VerificationSocket {
    pub fn new(runtime: &std::path::Path) -> std::io::Result<Self> {
        use std::os::unix::fs::DirBuilderExt;
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let directory = runtime.join(format!("primer-verify-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::DirBuilder::new().mode(0o700).create(&directory)?;
        match std::os::unix::net::UnixListener::bind(directory.join("ask")) {
            Ok(listener) => Ok(Self { directory, listener }),
            Err(error) => {
                let _ = std::fs::remove_dir(&directory);
                Err(error)
            }
        }
    }

    pub fn path(&self) -> std::path::PathBuf {
        self.directory.join("ask")
    }
}

impl Drop for VerificationSocket {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.path());
        let _ = std::fs::remove_dir(&self.directory);
    }
}

/// Run a bounded OpenSSH handshake using the editor as its askpass helper.
/// Authentication is checked again using the normal strict SFTP preflight.
pub fn verify_host(
    runtime: &std::path::Path,
    destination: &str,
    present: impl FnOnce(PendingConfirmation),
) -> anyhow::Result<()> {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    anyhow::ensure!(super::valid_destination(destination), "Invalid SSH destination");
    let socket = VerificationSocket::new(runtime)?;
    socket.listener.set_nonblocking(true)?;
    let mut child = Command::new("timeout")
        .args(["150s", "ssh", "-T", "-o", "StrictHostKeyChecking=ask",
            "-o", "BatchMode=no", "-o", "NumberOfPasswordPrompts=0",
            "-o", "PasswordAuthentication=no", "-o", "KbdInteractiveAuthentication=no",
            "-o", "PubkeyAuthentication=no", "-o", "PreferredAuthentications=none",
            "-o", "HostbasedAuthentication=no", "-o", "ControlMaster=no",
            "-o", "ControlPath=none", "-o", "ConnectTimeout=10",
            "-o", "FingerprintHash=sha256", "-o", "UpdateHostKeys=no", "-o", "ClearAllForwardings=yes", "-o", "PermitLocalCommand=no", "-N", "--", destination])
        .env("LC_ALL", "C")
        .env("SSH_ASKPASS", std::env::current_exe()?)
        .env("SSH_ASKPASS_REQUIRE", "force")
        .env("PRIMER_HOST_CONFIRMATION_SOCKET", socket.path())
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped())
        .spawn()?;
    let mut stderr = child.stderr.take().expect("SSH stderr is piped");
    let diagnostic_reader = std::thread::spawn(move || {
        use std::io::Read;
        let mut diagnostic = Vec::new();
        let _ = stderr.read_to_end(&mut diagnostic);
        String::from_utf8_lossy(&diagnostic).trim().to_owned()
    });
    let result = (|| -> anyhow::Result<()> {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            match socket.listener.accept() {
                Ok((stream, _)) => {
                    serve_confirmation(stream, present)?;
                    return Ok(());
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    anyhow::ensure!(child.try_wait()?.is_none(), "SSH exited without requesting host confirmation");
                    anyhow::ensure!(Instant::now() < deadline, "SSH host confirmation timed out");
                    std::thread::sleep(Duration::from_millis(25));
                }
                Err(error) => return Err(error.into()),
            }
        }
    })();
    if result.is_err() { let _ = child.kill(); }
    let _ = child.wait();
    let diagnostic = diagnostic_reader.join().unwrap_or_default();
    result.map_err(|error| {
        if diagnostic.is_empty() { error } else { anyhow::anyhow!("{error:#}: {diagnostic}") }
    })
}

/// Serve one confirmation over a private Unix socket connection.
/// The caller owns the listener, process lifetime and private socket directory.
pub fn serve_confirmation(
    mut stream: std::os::unix::net::UnixStream,
    present: impl FnOnce(PendingConfirmation),
) -> std::io::Result<()> {
    use std::io::{Read, Write};
    use std::time::Duration;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    let mut length = [0; 4];
    stream.read_exact(&mut length)?;
    let length = u32::from_be_bytes(length) as usize;
    if length > 16384 {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "SSH prompt too large"));
    }
    let mut request = vec![0; length];
    stream.read_exact(&mut request)?;
    let request = String::from_utf8(request)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    let confirmation = request.split_once('\0')
        .and_then(|(kind, prompt)| parse_confirmation(kind, prompt));
    let Some(confirmation) = confirmation else {
        return stream.write_all(b"no");
    };
    let (pending, mut receiver) = confirmation_channel(confirmation);
    let fingerprint = pending.confirmation.fingerprint.clone();
    present(pending);
    let deadline = std::time::Instant::now() + Duration::from_secs(120);
    let decision = loop {
        match receiver.try_recv() {
            Ok(decision) => break decision,
            Err(tokio::sync::oneshot::error::TryRecvError::Closed) => break HostDecision::Reject,
            Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                if std::time::Instant::now() >= deadline { break HostDecision::Reject; }
                std::thread::sleep(Duration::from_millis(25));
            }
        }
    };
    let confirmation = HostConfirmation { fingerprint };
    stream.write_all(confirmation.response(decision).as_bytes())
}

/// Askpass-side request. No shell evaluation and no terminal input.
pub fn request_confirmation(
    socket: &std::path::Path,
    kind: &str,
    prompt: &str,
) -> std::io::Result<String> {
    use std::io::{Read, Write};
    use std::time::Duration;
    if parse_confirmation(kind, prompt).is_none() { return Ok("no".into()); }
    let request = format!("{kind}\0{prompt}");
    if request.len() > 16384 { return Ok("no".into()); }
    let mut stream = std::os::unix::net::UnixStream::connect(socket)?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    stream.set_read_timeout(Some(Duration::from_secs(130)))?;
    stream.write_all(&(request.len() as u32).to_be_bytes())?;
    stream.write_all(request.as_bytes())?;
    let mut response = String::new();
    stream.take(128).read_to_string(&mut response)?;
    let confirmation = parse_confirmation(kind, prompt).unwrap();
    if response == confirmation.fingerprint { Ok(response) } else { Ok("no".into()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROMPT: &str = "The authenticity of host 'example (192.0.2.1)' can't be established.\nED25519 key fingerprint is SHA256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA.\nAre you sure you want to continue connecting (yes/no/[fingerprint])? ";

    #[test]
    fn private_socket_round_trip_and_cleanup() {
        use std::os::unix::fs::PermissionsExt;
        let runtime = tempfile::tempdir().unwrap();
        let socket = VerificationSocket::new(runtime.path()).unwrap();
        let path = socket.path();
        assert_eq!(std::fs::metadata(path.parent().unwrap()).unwrap().permissions().mode() & 0o777, 0o700);
        let request_path = path.clone();
        let client = std::thread::spawn(move || request_confirmation(&request_path, "confirm", PROMPT).unwrap());
        let (stream, _) = socket.listener.accept().unwrap();
        serve_confirmation(stream, |pending| {
            pending.decision.send(HostDecision::Trust).unwrap();
        }).unwrap();
        assert_eq!(client.join().unwrap(), parse_confirmation("confirm", PROMPT).unwrap().fingerprint);
        drop(socket);
        assert!(!path.parent().unwrap().exists());
    }

    #[test]
    fn decisions_pin_the_presented_fingerprint() {
        let confirmation = parse_confirmation("confirm", PROMPT).unwrap();
        assert_eq!(confirmation.response(HostDecision::Trust), confirmation.fingerprint);
        assert_eq!(confirmation.response(HostDecision::Reject), "no");
        assert_eq!(confirmation.response(HostDecision::Fingerprint("wrong".into())), "no");
        assert_eq!(confirmation.response(HostDecision::Fingerprint(confirmation.fingerprint.clone())), confirmation.fingerprint);
    }

    #[test]
    fn dropping_confirmation_cancels_the_channel() {
        let (pending, mut receiver) = confirmation_channel(parse_confirmation("confirm", PROMPT).unwrap());
        drop(pending);
        assert!(matches!(receiver.try_recv(), Err(tokio::sync::oneshot::error::TryRecvError::Closed)));
    }

    #[test]
    fn accepts_openssh_host_confirmation() {
        assert_eq!(parse_confirmation("", PROMPT), parse_confirmation("confirm", PROMPT));
        assert_eq!(parse_confirmation("confirm", PROMPT).unwrap().fingerprint,
            "SHA256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
    }

    #[test]
    fn rejects_other_prompt_types_and_malformed_fingerprints() {
        assert!(parse_confirmation("", "Password: ").is_none());
        assert!(parse_confirmation("none", PROMPT).is_none());
        assert!(parse_confirmation("confirm", "Password:").is_none());
        assert!(parse_confirmation("confirm", &PROMPT.replace("SHA256:", "MD5:")).is_none());
        assert!(parse_confirmation("confirm", &PROMPT.replace("AAA.", "AA!.")).is_none());
    }
}

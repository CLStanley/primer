//! Host confirmation data from OpenSSH's askpass interface.
//! Never treat password or keyboard-interactive prompts as host confirmation.

#[derive(Debug, PartialEq, Eq)]
pub struct HostConfirmation {
    pub fingerprint: String,
}

/// Accept only OpenSSH's confirmation prompt type and SHA256 host fingerprint.
/// The eventual response must be returned to the same waiting SSH process.
pub fn parse_confirmation(kind: &str, prompt: &str) -> Option<HostConfirmation> {
    if kind != "confirm"
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

#[cfg(test)]
mod tests {
    use super::*;

    const PROMPT: &str = "The authenticity of host 'example (192.0.2.1)' can't be established.\nED25519 key fingerprint is SHA256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA.\nAre you sure you want to continue connecting (yes/no/[fingerprint])? ";

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
        assert_eq!(parse_confirmation("confirm", PROMPT).unwrap().fingerprint,
            "SHA256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
    }

    #[test]
    fn rejects_other_prompt_types_and_malformed_fingerprints() {
        assert!(parse_confirmation("", PROMPT).is_none());
        assert!(parse_confirmation("none", PROMPT).is_none());
        assert!(parse_confirmation("confirm", "Password:").is_none());
        assert!(parse_confirmation("confirm", &PROMPT.replace("SHA256:", "MD5:")).is_none());
        assert!(parse_confirmation("confirm", &PROMPT.replace("AAA.", "AA!.")).is_none());
    }
}

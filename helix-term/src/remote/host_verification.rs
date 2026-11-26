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

#[cfg(test)]
mod tests {
    use super::*;

    const PROMPT: &str = "The authenticity of host 'example (192.0.2.1)' can't be established.\nED25519 key fingerprint is SHA256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA.\nAre you sure you want to continue connecting (yes/no/[fingerprint])? ";

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

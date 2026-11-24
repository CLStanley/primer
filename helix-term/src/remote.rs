//! OpenSSH connection metadata. Authentication remains the responsibility of OpenSSH.
#[cfg(target_os = "linux")]
pub mod mount;
#[cfg(target_os = "linux")]
pub mod registry;

use std::collections::BTreeSet;
use std::path::Path;
use anyhow::{ensure, Result};

/// Extract literal aliases, not wildcard or negated Host patterns.
/// OpenSSH itself resolves the effective configuration when connecting.
pub fn host_aliases(config: &str) -> Vec<String> {
    let mut aliases = BTreeSet::new();
    for line in config.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        let Some((keyword, values)) = line.split_once(|c: char| c.is_whitespace() || c == '=') else {
            continue;
        };
        if !keyword.eq_ignore_ascii_case("host") {
            continue;
        }
        for alias in values.trim_start_matches(|c: char| c.is_whitespace() || c == '=').split_whitespace() {
            let alias = alias.trim_matches('"');
            if valid_destination(alias) && !alias.contains('@') {
                aliases.insert(alias.to_owned());
            }
        }
    }
    aliases.into_iter().collect()
}

/// Conservative destination syntax for direct argv use; never accept options.
pub fn valid_destination(value: &str) -> bool {
    if value.is_empty() || value.starts_with('-') {
        return false;
    }
    let mut parts = value.split('@');
    let first = parts.next().unwrap_or("");
    let second = parts.next();
    if parts.next().is_some() {
        return false;
    }
    let valid_part = |part: &str| {
        !part.is_empty() && !part.starts_with('-')
            && part.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
    };
    match second {
        Some(host) => valid_part(first) && valid_part(host),
        None => valid_part(first),
    }
}

pub fn read_host_aliases(config: &Path) -> std::io::Result<Vec<String>> {
    match std::fs::read_to_string(config) {
        Ok(content) => Ok(host_aliases(&content)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(err) => Err(err),
    }
}

/// Construct an SSHFS source without shell interpolation.
/// Initial scope: Unix remote paths and hostname/alias destinations.
pub fn mount_source(destination: &str, remote_path: &str) -> Result<String> {
    ensure!(valid_destination(destination), "Invalid SSH destination");
    ensure!(remote_path.starts_with('/') && !remote_path.chars().any(char::is_control),
        "Remote path must be an absolute path without control characters");
    Ok(format!("{destination}:{remote_path}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_literal_hosts_only() {
        assert_eq!(
            host_aliases("Host nas debian *.example !excluded\n  HostName example\nhOsT=nas mac\n# Host ignored\n"),
            vec!["debian", "mac", "nas"]
        );
    }

    #[test]
    fn rejects_options_and_shell_syntax() {
        for value in ["", "-oProxyCommand=x", "host;cmd", "a b", "a@@b", "user@-host", "*"] {
            assert!(!valid_destination(value), "{value}");
        }
        assert!(valid_destination("user@debian.local"));
    }

    #[test]
    fn mount_source_preserves_spaces() {
        assert_eq!(mount_source("nas", "/home/user/my project").unwrap(), "nas:/home/user/my project");
        assert!(mount_source("nas", "relative").is_err());
        assert!(mount_source("-host", "/").is_err());
    }
}

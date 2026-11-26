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

/// OpenSSH passes remote commands through the remote login shell. Quote the
/// directory as a single POSIX shell word, including embedded apostrophes.
pub fn terminal_args(destination: &str, remote_path: &str) -> Result<Vec<String>> {
    mount_source(destination, remote_path)?;
    let quoted = format!("'{}'", remote_path.replace('\'', "'\\''"));
    Ok(vec![
        "-t".into(), "-o".into(), "StrictHostKeyChecking=yes".into(),
        "-o".into(), "BatchMode=yes".into(), "-o".into(), "ConnectTimeout=10".into(),
        "--".into(), destination.into(),
        format!("cd {quoted} && exec \"$SHELL\" -l"),
    ])
}


/// Probe the same SFTP subsystem used by SSHFS, without opening a shell.
#[cfg(target_os = "linux")]
pub fn ssh_auth_preflight(destination: &str) -> Result<()> {
    use std::process::{Command, Stdio};
    ensure!(valid_destination(destination), "Invalid SSH destination");
    let output = Command::new("timeout")
        .args(["20s", "sftp", "-b", "/dev/null", "-o", "BatchMode=yes",
            "-o", "StrictHostKeyChecking=yes", "-o", "ConnectTimeout=10", "--", destination])
        .stdin(Stdio::null()).output()?;
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    ensure!(output.status.success(), "SSH authentication/SFTP check failed: {}",
        if diagnostic.trim().is_empty() { "Connection timed out or was rejected" } else { diagnostic.trim() });
    Ok(())
}

/// Recognize OpenSSH's strict-checking rejection of a previously unknown key.
/// Never offer enrollment for a changed or revoked key.
#[cfg(target_os = "linux")]
pub fn unknown_host_key(diagnostic: &str) -> bool {
    !diagnostic.contains("REMOTE HOST IDENTIFICATION HAS CHANGED")
        && !diagnostic.contains("REVOKED HOST KEY")
        && diagnostic.lines().any(|line| {
            let line = line.trim().strip_prefix("SSH authentication/SFTP check failed: ").unwrap_or(line.trim());
            line.starts_with("No ")
                && line.contains(" host key is known for ")
                && line.contains("strict checking")
        })
        && diagnostic.contains("Host key verification failed")
}

/// Accept only the official Tailscale login origin, never arbitrary server URLs.
#[cfg(target_os = "linux")]
pub fn authentication_url(diagnostic: &str) -> Option<String> {
    diagnostic.split_whitespace().find_map(|word| {
        let url = word.trim_matches(|c: char| matches!(c, '(' | ')' | '<' | '>' | '"' | '\'' | ',' | '.'));
        let path = url.strip_prefix("https://login.tailscale.com/")?;
        if path.is_empty() || !path.bytes().all(|b| b.is_ascii_alphanumeric() || b"/-_.?=&%".contains(&b)) {
            return None;
        }
        Some(url.to_owned())
    })
}

/// Optional Tailscale diagnostics. Unknown hosts are never assumed to be tailnet peers.
#[cfg(target_os = "linux")]
pub fn tailscale_preflight(destination: &str) -> Result<()> {
    use std::process::Command;
    let host = destination.rsplit('@').next().unwrap_or(destination);
    let effective = Command::new("ssh").args(["-G", "--", destination]).output().ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|output| output.lines().find_map(|line| line.strip_prefix("hostname ").map(str::to_owned)))
        .unwrap_or_else(|| host.to_owned());
    let explicit = effective.trim_end_matches('.').ends_with(".ts.net")
        || effective.parse::<std::net::Ipv4Addr>().is_ok_and(|ip| {
            let octets = ip.octets();
            octets[0] == 100 && (64..=127).contains(&octets[1])
        });
    let output = match Command::new("tailscale").args(["status", "--json"]).output() {
        Ok(output) => output,
        Err(error) if explicit => return Err(error).map_err(|error| anyhow::anyhow!("Tailscale CLI unavailable: {error}")),
        Err(_) => return Ok(()),
    };
    let status: serde_json::Value = match serde_json::from_slice(&output.stdout) {
        Ok(status) => status,
        Err(_) if explicit => anyhow::bail!("Cannot read Tailscale status. Check that tailscaled is running."),
        Err(_) => return Ok(()),
    };
    check_tailscale_status(&status, &effective, explicit)
}

#[cfg(target_os = "linux")]
fn check_tailscale_status(status: &serde_json::Value, host: &str, explicit: bool) -> Result<()> {
    let host = host.trim_end_matches('.');
    let peer = status["Peer"].as_object().and_then(|peers| peers.values().find(|peer| {
        peer["HostName"].as_str().is_some_and(|name| name.eq_ignore_ascii_case(host))
            || peer["DNSName"].as_str().is_some_and(|name| name.trim_end_matches('.').eq_ignore_ascii_case(host))
            || peer["TailscaleIPs"].as_array().is_some_and(|ips| ips.iter().any(|ip| ip.as_str() == Some(host)))
    }));
    if !explicit && peer.is_none() { return Ok(()); }
    match status["BackendState"].as_str() {
        Some("NeedsLogin") => anyhow::bail!("Tailscale authentication required. {}", status["AuthURL"].as_str().unwrap_or("Run tailscale login and complete browser authentication, then retry.")),
        Some("NeedsMachineAuth") => anyhow::bail!("Tailscale device approval required. Approve this device in the Tailscale admin console, then retry."),
        Some("Stopped") => anyhow::bail!("Tailscale is disconnected. Run tailscale up, then retry."),
        Some("Running") => Ok(()),
        _ => anyhow::bail!("Tailscale is not ready. Check tailscale status and the tailscaled service."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn tailscale_checks_only_known_or_explicit_hosts() {
        let status = serde_json::json!({"BackendState":"NeedsLogin","Peer":{"node":{"HostName":"desktop"}}});
        assert!(check_tailscale_status(&status, "desktop", false).is_err());
        assert!(check_tailscale_status(&status, "other", false).is_ok());
        assert!(check_tailscale_status(&status, "host.ts.net", true).is_err());
        assert!(check_tailscale_status(&serde_json::json!({"BackendState":"Running"}), "host.ts.net", true).is_ok());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn unknown_host_key_recovery_excludes_changed_keys() {
        let unknown = "SSH authentication/SFTP check failed: No ED25519 host key is known for example and you have requested strict checking.\nHost key verification failed.";
        // The preflight wraps the first diagnostic line with context.
        assert!(unknown_host_key(unknown));
        assert!(!unknown_host_key("WARNING: REMOTE HOST IDENTIFICATION HAS CHANGED!\nHost key verification failed."));
        assert!(!unknown_host_key("Host key verification failed."));
        assert!(!unknown_host_key("Permission denied (publickey)."));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn authentication_urls_require_official_origin() {
        assert_eq!(authentication_url("Visit https://login.tailscale.com/a/123"), Some("https://login.tailscale.com/a/123".into()));
        assert!(authentication_url("https://login.tailscale.com.evil.test/a").is_none());
        assert!(authentication_url("https://evil.test/a").is_none());
    }

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
    fn terminal_command_quotes_remote_directory() {
        let args = terminal_args("user@nas", "/home/user/a b").unwrap();
        assert_eq!(args.last().unwrap(), "cd '/home/user/a b' && exec \"$SHELL\" -l");
        let args = terminal_args("nas", "/a'b;$(id)").unwrap();
        assert_eq!(args.last().unwrap(), "cd '/a'\\''b;$(id)' && exec \"$SHELL\" -l");
        assert!(terminal_args("-host", "/").is_err());
        assert!(terminal_args("nas", "relative").is_err());
    }

    #[test]
    fn mount_source_preserves_spaces() {
        assert_eq!(mount_source("nas", "/home/user/my project").unwrap(), "nas:/home/user/my project");
        assert!(mount_source("nas", "relative").is_err());
        assert!(mount_source("-host", "/").is_err());
    }
}

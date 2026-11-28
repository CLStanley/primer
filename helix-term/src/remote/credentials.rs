//! Optional system keyring storage. Never fall back to plaintext files.
use anyhow::{ensure, Result};
use secret_service::{blocking::SecretService, EncryptionType};
use std::collections::HashMap;

fn attributes(destination: &str) -> Result<HashMap<&str, &str>> {
    ensure!(super::valid_destination(destination), "Invalid SSH destination");
    Ok(HashMap::from([("application", "primer"), ("purpose", "ssh-password"), ("destination", destination)]))
}

/// Call on a blocking worker, never on the editor thread.
pub fn available() -> bool {
    SecretService::connect(EncryptionType::Dh)
        .and_then(|service| service.get_default_collection().and_then(|collection| collection.is_locked()))
        .is_ok_and(|locked| !locked)
}

pub fn retrieve(destination: &str) -> Result<Option<String>> {
    let attributes = attributes(destination)?;
    let service = SecretService::connect(EncryptionType::Dh)?;
    let collection = service.get_default_collection()?;
    ensure!(!collection.is_locked()?, "System keyring is locked");
    let items = collection.search_items(attributes)?;
    ensure!(items.len() <= 1, "Multiple matching SSH credentials");
    let Some(item) = items.first() else { return Ok(None); };
    ensure!(!item.is_locked()?, "SSH credential is locked");
    let password = String::from_utf8(item.get_secret()?)?;
    ensure!(password.len() <= 16384 && !password.contains(['\n', '\r', '\0']), "Invalid stored SSH credential");
    Ok(Some(password))
}

pub fn store(destination: &str, password: &str) -> Result<()> {
    let attributes = attributes(destination)?;
    ensure!(password.len() <= 16384 && !password.contains(['\n', '\r', '\0']), "Invalid SSH credential");
    let service = SecretService::connect(EncryptionType::Dh)?;
    let collection = service.get_default_collection()?;
    ensure!(!collection.is_locked()?, "System keyring is locked");
    collection.create_item(&format!("Primer SSH: {destination}"), attributes, password.as_bytes(), true, "text/plain")?;
    Ok(())
}

pub fn forget(destination: &str) -> Result<()> {
    let attributes = attributes(destination)?;
    let service = SecretService::connect(EncryptionType::Dh)?;
    let collection = service.get_default_collection()?;
    ensure!(!collection.is_locked()?, "System keyring is locked");
    for item in collection.search_items(attributes)? {
        ensure!(!item.is_locked()?, "SSH credential is locked");
        item.delete()?;
    }
    Ok(())
}

/// Check metadata only; do not retrieve the password to render connection status.
pub fn has_saved(destination: &str) -> Result<bool> {
    let attributes = attributes(destination)?;
    let service = SecretService::connect(EncryptionType::Dh)?;
    let collection = service.get_default_collection()?;
    ensure!(!collection.is_locked()?, "System keyring is locked");
    let has_saved = !collection.search_items(attributes)?.is_empty();
    Ok(has_saved)
}

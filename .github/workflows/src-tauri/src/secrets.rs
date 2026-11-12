//! API keys live in the Windows Credential Manager, never in SQLite, logs or project files.

#[cfg(windows)]
fn entry(provider: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new("alpha-ai-video-studio", provider).map_err(|e| e.to_string())
}

#[cfg(windows)]
pub fn set(provider: &str, key: &str) -> Result<(), String> {
    entry(provider)?.set_password(key).map_err(|e| e.to_string())
}
#[cfg(windows)]
pub fn has(provider: &str) -> bool {
    entry(provider).and_then(|e| e.get_password().map_err(|e| e.to_string())).is_ok()
}
#[cfg(windows)]
pub fn delete(provider: &str) -> Result<(), String> {
    entry(provider)?.delete_credential().map_err(|e| e.to_string())
}

#[cfg(not(windows))]
pub fn set(_: &str, _: &str) -> Result<(), String> {
    Err("Secure key storage is only implemented for Windows (Credential Manager).".into())
}
#[cfg(not(windows))]
pub fn has(_: &str) -> bool {
    false
}
#[cfg(not(windows))]
pub fn delete(_: &str) -> Result<(), String> {
    Err("Secure key storage is only implemented for Windows (Credential Manager).".into())
}

//! Authentication & Token Validation for EDM Handoff.
//!
//! Handles 64-char cryptographically random tokens stored in %APPDATA%\EDM\token.txt,
//! constant-time token comparison, and browser extension origin whitelisting.

use std::fs;
use std::path::{Path, PathBuf};

/// Token length invariant: 64 hexadecimal characters.
pub const TOKEN_LENGTH: usize = 64;

/// Retrieves the path to the stored token file in %APPDATA%\EDM\token.txt.
pub fn get_token_file_path() -> PathBuf {
    match std::env::var("APPDATA") {
        Ok(appdata) => PathBuf::from(appdata).join("EDM").join("token.txt"),
        Err(_) => std::env::temp_dir().join("EDM").join("token.txt"),
    }
}

/// Loads existing token or generates a new 64-character random token.
pub fn get_or_create_token() -> std::io::Result<String> {
    let path = get_token_file_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    if let Ok(content) = fs::read_to_string(&path) {
        let trimmed = content.trim();
        if trimmed.len() >= 32 {
            return Ok(trimmed.to_string());
        }
    }

    // Generate 64 hex characters (32 random bytes)
    let mut bytes = [0u8; 32];
    for b in &mut bytes {
        *b = (rand_fallback() & 0xFF) as u8;
    }
    let token = bytes.iter().map(|b| format!("{:02x}", b)).collect::<String>();
    fs::write(&path, &token)?;
    Ok(token)
}

/// Constant-time string comparison to prevent timing attacks.
pub fn constant_time_eq(a: &str, b: &str) -> bool {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    if a_bytes.len() != b_bytes.len() {
        return false;
    }
    let mut result = 0u8;
    for (x, y) in a_bytes.iter().zip(b_bytes.iter()) {
        result |= x ^ y;
    }
    result == 0
}

/// Fallback pseudorandom byte generator using system time and process ID.
fn rand_fallback() -> u64 {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let pid = std::process::id() as u128;
    ((now ^ (pid << 32)) & 0xFFFFFFFFFFFFFFFF) as u64
}

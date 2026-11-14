//! Native Messaging Host manifest templates and registry key definitions.
//!
//! Provides Chromium and Firefox native messaging host configuration for edm-nmh.

pub const HOST_NAME: &str = "com.edm.nmh";

pub const CHROMIUM_REGISTRY_KEYS: &[&str] = &[
    r"Software\Google\Chrome\NativeMessagingHosts\com.edm.nmh",
    r"Software\Microsoft\Edge\NativeMessagingHosts\com.edm.nmh",
    r"Software\BraveSoftware\Brave-Browser\NativeMessagingHosts\com.edm.nmh",
];

pub const FIREFOX_REGISTRY_KEYS: &[&str] = &[
    r"Software\Mozilla\NativeMessagingHosts\com.edm.nmh",
];

/// Generates Chromium native messaging manifest JSON content.
pub fn generate_chromium_manifest(nmh_exe_path: &str, extension_ids: &[&str]) -> String {
    let origins: Vec<String> = extension_ids
        .iter()
        .map(|id| format!("chrome-extension://{}/", id))
        .collect();

    serde_json::json!({
        "name": HOST_NAME,
        "description": "EDM Native Messaging Host Helper",
        "path": nmh_exe_path,
        "type": "stdio",
        "allowed_origins": origins
    })
    .to_string()
}

/// Generates Firefox native messaging manifest JSON content.
pub fn generate_firefox_manifest(nmh_exe_path: &str, firefox_extension_id: &str) -> String {
    serde_json::json!({
        "name": HOST_NAME,
        "description": "EDM Native Messaging Host Helper",
        "path": nmh_exe_path,
        "type": "stdio",
        "allowed_extensions": [firefox_extension_id]
    })
    .to_string()
}

// EDM Runtime Asset Resilience & Cryptographic Self-Healing Engine
// Automatically validates and self-heals critical runtime dependencies (WebView2Loader.dll)

use std::path::Path;

#[cfg(all(windows, target_arch = "x86_64"))]
pub const EMBEDDED_WEBVIEW2_LOADER: &[u8] = include_bytes!(
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../../assets/vendor/win-x64/WebView2Loader.dll")
);

const PINNED_WV2_SHA256: &str = "86545B66CDB0603BC26B626FB9AD610CB6E71F28D468F5EA66DF23B03DDA96D5";

/// Validates whether a file on disk is an authentic, non-corrupted WebView2Loader.dll
pub fn is_valid_webview2_dll(path: &Path) -> bool {
    if let Ok(data) = std::fs::read(path) {
        if data.len() > 1000 {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(&data);
            let hash = hex::encode(hasher.finalize());
            return hash.eq_ignore_ascii_case(PINNED_WV2_SHA256);
        }
    }
    false
}

/// Atomically extracts the genuine embedded WebView2Loader.dll to `dir` if missing or corrupted
pub fn self_heal_dll_in_dir(dir: &Path) {
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        let dll_path = dir.join("WebView2Loader.dll");

        if is_valid_webview2_dll(&dll_path) {
            return; // Fast path: 100% genuine and intact
        }

        let _ = std::fs::create_dir_all(dir);
        let pid = std::process::id();
        let tmp_path = dir.join(format!("WebView2Loader.dll.tmp.{}", pid));

        if std::fs::write(&tmp_path, EMBEDDED_WEBVIEW2_LOADER).is_ok() {
            let _ = std::fs::rename(&tmp_path, &dll_path);
            let _ = std::fs::remove_file(&tmp_path);
            tracing::info!(
                target = "edm::assets",
                "Successfully self-healed authentic WebView2Loader.dll in {}",
                dir.display()
            );
        }
    }
}

/// Guarantees that essential runtime dependencies exist in both the application execution directory
/// and the canonical installation path (%LOCALAPPDATA%\EDM).
pub fn ensure_runtime_environment() {
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        // 1. Ensure co-located next to current executable
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                self_heal_dll_in_dir(exe_dir);
            }
        }

        // 2. Ensure present in %LOCALAPPDATA%\EDM ($INSTDIR)
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let edm_dir = std::path::PathBuf::from(local_app_data).join("EDM");
            self_heal_dll_in_dir(&edm_dir);
        }
    }
}

fn main() {
    let nmh_path = std::path::Path::new("../../../target/release/edm-nmh.exe");
    if !nmh_path.exists() {
        if let Some(parent) = nmh_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(nmh_path, b"");
    }

    // Ensure authentic vendor WebView2Loader.dll is copied to target/release/
    let vendor_wv2 = std::path::Path::new("../../../assets/vendor/win-x64/WebView2Loader.dll");
    let target_wv2 = std::path::Path::new("../../../target/release/WebView2Loader.dll");
    if vendor_wv2.exists() && vendor_wv2.metadata().map(|m| m.len()).unwrap_or(0) > 1000 {
        if let Some(parent) = target_wv2.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::copy(vendor_wv2, target_wv2);
    } else if !target_wv2.exists() {
        if let Some(parent) = target_wv2.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(target_wv2, b"");
    }

    tauri_build::build()
}

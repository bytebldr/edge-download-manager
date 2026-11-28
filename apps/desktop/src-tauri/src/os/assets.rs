// EDM Runtime Asset Resilience & Cryptographic Self-Healing Engine
// Automatically validates and self-heals critical runtime dependencies (WebView2Loader.dll)

use std::path::Path;

#[cfg(all(windows, target_arch = "x86_64"))]
pub const EMBEDDED_WEBVIEW2_LOADER: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../assets/vendor/win-x64/WebView2Loader.dll"
));

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

        // 3. Cap WebView2 V8 heap & disk/media caches to keep flyout & main window lean
        if std::env::var_os("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS").is_none() {
            std::env::set_var(
                "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
                "--disable-gpu-shader-disk-cache --disk-cache-size=1048576 --media-cache-size=1048576 --js-flags=--max-old-space-size=64",
            );
        }

        // 4. Ensure EDM's Windows 11 system tray icon is promoted (not hidden in overflow)
        std::thread::spawn(promote_windows_tray_icon);
    }
}

/// Ensures EDM's Windows 11 system tray icon is pinned visible in the taskbar
/// (`HKCU\Control Panel\NotifyIconSettings\<id>\IsPromoted = 1`).
#[cfg(windows)]
pub fn promote_windows_tray_icon() {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let ps = r#"
$base = 'HKCU:\Control Panel\NotifyIconSettings'
if (Test-Path $base) {
  Get-ChildItem $base -ErrorAction SilentlyContinue | ForEach-Object {
    $exe = (Get-ItemProperty $_.PSPath -Name ExecutablePath -ErrorAction SilentlyContinue).ExecutablePath
    if ($exe -and ($exe -like '*\EDM.exe' -or $exe -like '*\edm-desktop-app.exe')) {
      $cur = (Get-ItemProperty $_.PSPath -Name IsPromoted -ErrorAction SilentlyContinue).IsPromoted
      if ($cur -ne 1) {
        Set-ItemProperty -Path $_.PSPath -Name IsPromoted -Value 1 -Type DWord -ErrorAction SilentlyContinue
      }
    }
  }
}
"#;
    let _ = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", ps])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
}

/// Asks the Windows memory manager to trim unused working-set pages from the
/// current process AND its child WebView2 processes, reclaiming memory after
/// windows are destroyed, minimized, or transitioning into idle/background states.
pub fn trim_process_working_set() -> bool {
    #[cfg(windows)]
    {
        #[repr(C)]
        struct ProcessEntry32W {
            dw_size: u32,
            cnt_usage: u32,
            th32_process_id: u32,
            th32_default_heap_id: usize,
            th32_module_id: u32,
            cnt_threads: u32,
            th32_parent_process_id: u32,
            pc_pri_class_base: i32,
            dw_flags: u32,
            sz_exe_file: [u16; 260],
        }

        #[link(name = "kernel32")]
        extern "system" {
            fn GetCurrentProcess() -> *mut std::ffi::c_void;
            fn GetCurrentProcessId() -> u32;
            fn SetProcessWorkingSetSize(
                h_process: *mut std::ffi::c_void,
                dw_minimum_working_set_size: usize,
                dw_maximum_working_set_size: usize,
            ) -> i32;
            fn CreateToolhelp32Snapshot(
                dw_flags: u32,
                th32_process_id: u32,
            ) -> *mut std::ffi::c_void;
            fn Process32FirstW(
                h_snapshot: *mut std::ffi::c_void,
                lppe: *mut ProcessEntry32W,
            ) -> i32;
            fn Process32NextW(h_snapshot: *mut std::ffi::c_void, lppe: *mut ProcessEntry32W)
                -> i32;
            fn OpenProcess(
                dw_desired_access: u32,
                b_inherit_handle: i32,
                dw_process_id: u32,
            ) -> *mut std::ffi::c_void;
            fn CloseHandle(h_object: *mut std::ffi::c_void) -> i32;
        }

        const TH32CS_SNAPPROCESS: u32 = 0x0000_0002;
        const PROCESS_SET_QUOTA: u32 = 0x0100;
        const PROCESS_QUERY_INFORMATION: u32 = 0x0400;

        unsafe {
            let self_ok =
                SetProcessWorkingSetSize(GetCurrentProcess(), usize::MAX, usize::MAX) != 0;

            let my_pid = GetCurrentProcessId();
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if !snap.is_null() && snap as isize != -1 {
                let mut pe: ProcessEntry32W = std::mem::zeroed();
                pe.dw_size = std::mem::size_of::<ProcessEntry32W>() as u32;
                if Process32FirstW(snap, &mut pe) != 0 {
                    loop {
                        if pe.th32_parent_process_id == my_pid {
                            let h_child = OpenProcess(
                                PROCESS_SET_QUOTA | PROCESS_QUERY_INFORMATION,
                                0,
                                pe.th32_process_id,
                            );
                            if !h_child.is_null() {
                                let _ = SetProcessWorkingSetSize(h_child, usize::MAX, usize::MAX);
                                let _ = CloseHandle(h_child);
                            }
                        }
                        if Process32NextW(snap, &mut pe) == 0 {
                            break;
                        }
                    }
                }
                let _ = CloseHandle(snap);
            }
            self_ok
        }
    }
    #[cfg(not(windows))]
    {
        false
    }
}

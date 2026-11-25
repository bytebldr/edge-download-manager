#![cfg(windows)]
#![allow(non_snake_case, non_upper_case_globals, dead_code)]

use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use windows_registry::CURRENT_USER;

#[repr(C)]
struct GUID {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

const CLSID_ShellLink: GUID = GUID {
    data1: 0x00021401,
    data2: 0x0000,
    data3: 0x0000,
    data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};
const IID_IShellLinkW: GUID = GUID {
    data1: 0x000214F9,
    data2: 0x0000,
    data3: 0x0000,
    data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};
const IID_IPersistFile: GUID = GUID {
    data1: 0x0000010b,
    data2: 0x0000,
    data3: 0x0000,
    data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};

const CLSCTX_INPROC_SERVER: u32 = 1;
const COINIT_APARTMENTTHREADED: u32 = 0x2;

#[repr(C)]
struct IUnknownVtbl {
    QueryInterface: unsafe extern "system" fn(this: *mut c_void, riid: *const GUID, ppv: *mut *mut c_void) -> i32,
    AddRef: unsafe extern "system" fn(this: *mut c_void) -> u32,
    Release: unsafe extern "system" fn(this: *mut c_void) -> u32,
}

#[repr(C)]
struct IShellLinkWVtbl {
    parent: IUnknownVtbl,
    GetPath: unsafe extern "system" fn(this: *mut c_void, pszFile: *mut u16, cch: i32, pfd: *mut c_void, fFlags: u32) -> i32,
    GetIDList: unsafe extern "system" fn(this: *mut c_void, ppidl: *mut *mut c_void) -> i32,
    SetIDList: unsafe extern "system" fn(this: *mut c_void, pidl: *const c_void) -> i32,
    GetDescription: unsafe extern "system" fn(this: *mut c_void, pszName: *mut u16, cch: i32) -> i32,
    SetDescription: unsafe extern "system" fn(this: *mut c_void, pszName: *const u16) -> i32,
    GetWorkingDirectory: unsafe extern "system" fn(this: *mut c_void, pszDir: *mut u16, cch: i32) -> i32,
    SetWorkingDirectory: unsafe extern "system" fn(this: *mut c_void, pszDir: *const u16) -> i32,
    GetArguments: unsafe extern "system" fn(this: *mut c_void, pszArgs: *mut u16, cch: i32) -> i32,
    SetArguments: unsafe extern "system" fn(this: *mut c_void, pszArgs: *const u16) -> i32,
    GetHotkey: unsafe extern "system" fn(this: *mut c_void, pwHotkey: *mut u16) -> i32,
    SetHotkey: unsafe extern "system" fn(this: *mut c_void, wHotkey: u16) -> i32,
    GetShowCmd: unsafe extern "system" fn(this: *mut c_void, piShowCmd: *mut i32) -> i32,
    SetShowCmd: unsafe extern "system" fn(this: *mut c_void, iShowCmd: i32) -> i32,
    GetIconLocation: unsafe extern "system" fn(this: *mut c_void, pszIconPath: *mut u16, cch: i32, piIcon: *mut i32) -> i32,
    SetIconLocation: unsafe extern "system" fn(this: *mut c_void, pszIconPath: *const u16, iIcon: i32) -> i32,
    SetRelativePath: unsafe extern "system" fn(this: *mut c_void, pszPathRel: *const u16, dwReserved: u32) -> i32,
    Resolve: unsafe extern "system" fn(this: *mut c_void, hwnd: *mut c_void, fFlags: u32) -> i32,
    SetPath: unsafe extern "system" fn(this: *mut c_void, pszFile: *const u16) -> i32,
}

#[repr(C)]
struct IPersistFileVtbl {
    parent: IUnknownVtbl,
    GetClassID: unsafe extern "system" fn(this: *mut c_void, pClassID: *mut GUID) -> i32,
    IsDirty: unsafe extern "system" fn(this: *mut c_void) -> i32,
    Load: unsafe extern "system" fn(this: *mut c_void, pszFileName: *const u16, dwMode: u32) -> i32,
    Save: unsafe extern "system" fn(this: *mut c_void, pszFileName: *const u16, fRemember: i32) -> i32,
    SaveCompleted: unsafe extern "system" fn(this: *mut c_void, pszFileName: *const u16) -> i32,
    GetCurFile: unsafe extern "system" fn(this: *mut c_void, ppszFileName: *mut *mut u16) -> i32,
}

extern "system" {
    fn CoInitializeEx(pvReserved: *mut c_void, dwCoInit: u32) -> i32;
    fn CoUninitialize();
    fn CoCreateInstance(
        rclsid: *const GUID,
        pUnkOuter: *mut c_void,
        dwClsContext: u32,
        riid: *const GUID,
        ppv: *mut *mut c_void,
    ) -> i32;

    fn CreateToolhelp32Snapshot(dwFlags: u32, th32ProcessID: u32) -> *mut c_void;
    fn Process32FirstW(hSnapshot: *mut c_void, lppe: *mut PROCESSENTRY32W) -> i32;
    fn Process32NextW(hSnapshot: *mut c_void, lppe: *mut PROCESSENTRY32W) -> i32;
    fn OpenProcess(dwDesiredAccess: u32, bInheritHandle: i32, dwProcessId: u32) -> *mut c_void;
    fn TerminateProcess(hProcess: *mut c_void, uExitCode: u32) -> i32;
    fn CloseHandle(hObject: *mut c_void) -> i32;
    fn GetCurrentProcessId() -> u32;
}

const TH32CS_SNAPPROCESS: u32 = 0x00000002;
const PROCESS_TERMINATE: u32 = 0x0001;
const INVALID_HANDLE_VALUE: *mut c_void = -1isize as *mut c_void;

#[repr(C)]
struct PROCESSENTRY32W {
    dwSize: u32,
    cntUsage: u32,
    th32ProcessID: u32,
    th32DefaultHeapID: usize,
    th32ModuleID: u32,
    cntThreads: u32,
    th32ParentProcessID: u32,
    pcPriClassBase: i32,
    dwFlags: u32,
    szExeFile: [u16; 260],
}

fn to_wide(s: impl AsRef<std::ffi::OsStr>) -> Vec<u16> {
    s.as_ref().encode_wide().chain(std::iter::once(0)).collect()
}

/// Create a Windows shortcut (.lnk) using pure Win32 COM (IShellLinkW & IPersistFile).
/// Completely eliminates any powershell.exe dependency or console flashes.
pub fn create_com_shortcut(
    shortcut_path: &Path,
    target_exe: &Path,
    work_dir: &Path,
    icon_path: Option<&Path>,
    icon_index: i32,
    args: Option<&str>,
) -> Result<(), String> {
    unsafe {
        let hr = CoInitializeEx(std::ptr::null_mut(), COINIT_APARTMENTTHREADED);
        let uninit_needed = hr >= 0;

        let mut shell_link_raw: *mut c_void = std::ptr::null_mut();
        let hr_inst = CoCreateInstance(
            &CLSID_ShellLink,
            std::ptr::null_mut(),
            CLSCTX_INPROC_SERVER,
            &IID_IShellLinkW,
            &mut shell_link_raw,
        );

        if hr_inst < 0 || shell_link_raw.is_null() {
            if uninit_needed {
                CoUninitialize();
            }
            return Err(format!("CoCreateInstance(CLSID_ShellLink) failed: 0x{:08X}", hr_inst as u32));
        }

        let link_vtbl = *(shell_link_raw as *mut *const IShellLinkWVtbl);
        let unk_vtbl = *(shell_link_raw as *mut *const IUnknownVtbl);

        let target_wide = to_wide(target_exe.as_os_str());
        ( (*link_vtbl).SetPath )(shell_link_raw, target_wide.as_ptr());

        let work_wide = to_wide(work_dir.as_os_str());
        ( (*link_vtbl).SetWorkingDirectory )(shell_link_raw, work_wide.as_ptr());

        if let Some(a) = args {
            let args_wide = to_wide(a);
            ( (*link_vtbl).SetArguments )(shell_link_raw, args_wide.as_ptr());
        }

        let icon_target = icon_path.unwrap_or(target_exe);
        let icon_wide = to_wide(icon_target.as_os_str());
        ( (*link_vtbl).SetIconLocation )(shell_link_raw, icon_wide.as_ptr(), icon_index);

        // QueryInterface for IPersistFile
        let mut persist_file_raw: *mut c_void = std::ptr::null_mut();
        let hr_qi = ( (*unk_vtbl).QueryInterface )(shell_link_raw, &IID_IPersistFile, &mut persist_file_raw);

        let save_res = if hr_qi >= 0 && !persist_file_raw.is_null() {
            let pf_vtbl = *(persist_file_raw as *mut *const IPersistFileVtbl);
            if let Some(parent) = shortcut_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let lnk_wide = to_wide(shortcut_path.as_os_str());
            let hr_save = ( (*pf_vtbl).Save )(persist_file_raw, lnk_wide.as_ptr(), 1);
            let pf_unk_vtbl = *(persist_file_raw as *mut *const IUnknownVtbl);
            ( (*pf_unk_vtbl).Release )(persist_file_raw);

            if hr_save >= 0 {
                Ok(())
            } else {
                Err(format!("IPersistFile::Save failed: 0x{:08X}", hr_save as u32))
            }
        } else {
            Err(format!("QueryInterface(IID_IPersistFile) failed: 0x{:08X}", hr_qi as u32))
        };

        ( (*unk_vtbl).Release )(shell_link_raw);

        if uninit_needed {
            CoUninitialize();
        }

        save_res
    }
}

/// Terminate sibling EDM and NMH processes via Win32 Toolhelp snapshot without child process flashing.
pub fn terminate_sibling_instances() -> Result<(), String> {
    unsafe {
        let current_pid = GetCurrentProcessId();
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return Err("Failed to create process snapshot".to_string());
        }

        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            cntUsage: 0,
            th32ProcessID: 0,
            th32DefaultHeapID: 0,
            th32ModuleID: 0,
            cntThreads: 0,
            th32ParentProcessID: 0,
            pcPriClassBase: 0,
            dwFlags: 0,
            szExeFile: [0; 260],
        };

        let target_names = ["edm.exe", "edm-desktop-app.exe", "edm-nmh.exe"];

        if Process32FirstW(snapshot, &mut entry) != 0 {
            loop {
                let pid = entry.th32ProcessID;
                if pid != current_pid && pid != 0 {
                    let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
                    let name = String::from_utf16_lossy(&entry.szExeFile[..len]).to_lowercase();
                    if target_names.iter().any(|&t| t == name) {
                        let proc_handle = OpenProcess(PROCESS_TERMINATE, 0, pid);
                        if !proc_handle.is_null() {
                            let _ = TerminateProcess(proc_handle, 0);
                            CloseHandle(proc_handle);
                        }
                    }
                }
                if Process32NextW(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }

        CloseHandle(snapshot);
        std::thread::sleep(std::time::Duration::from_millis(300));
        Ok(())
    }
}

/// Check if any file in the given paths is currently locked by a running process.
pub fn check_file_locked(path: &Path) -> bool {
    if !path.exists() {
        return false;
    }
    match std::fs::OpenOptions::new().write(true).open(path) {
        Ok(_) => false,
        Err(e) => {
            // Sharing violation (ERROR_SHARING_VIOLATION = 32) or PermissionDenied (5)
            e.kind() == std::io::ErrorKind::PermissionDenied || e.raw_os_error() == Some(32) || e.raw_os_error() == Some(5)
        }
    }
}

pub fn check_process_file_locks(paths: &[PathBuf]) -> bool {
    paths.iter().any(|p| check_file_locked(p))
}

pub fn register_autostart(edm_exe: &Path, enabled: bool) -> Result<(), String> {
    if let Ok(key) = CURRENT_USER.create(r"Software\Microsoft\Windows\CurrentVersion\Run") {
        if enabled {
            let val = format!("\"{}\" --background", edm_exe.to_string_lossy());
            let _ = key.set_string("EDM", &val);
        } else {
            let _ = key.remove_value("EDM");
        }
        Ok(())
    } else {
        Err("Cannot access Run registry key".into())
    }
}

pub fn register_uninstall_entry(target_dir: &Path, edm_exe: &Path) -> Result<(), String> {
    let uninstall_key = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\EDM";
    let key = CURRENT_USER.create(uninstall_key).map_err(|e| e.to_string())?;

    let edm_exe_str = edm_exe.to_string_lossy().to_string();
    let _ = key.set_string("DisplayName", "EDM — Evo Download Manager");
    let _ = key.set_string("DisplayIcon", &edm_exe_str);
    let _ = key.set_string("DisplayVersion", env!("CARGO_PKG_VERSION"));
    let _ = key.set_string("Publisher", "bytebldr");
    let now_date = chrono::Local::now().format("%Y%m%d").to_string();
    let _ = key.set_string("InstallDate", &now_date);
    let _ = key.set_string("UninstallString", &format!("\"{}\" --uninstall", edm_exe_str));
    let _ = key.set_string("QuietUninstallString", &format!("\"{}\" --clean-uninstall", edm_exe_str));
    let _ = key.set_string("InstallLocation", &target_dir.to_string_lossy());
    let _ = key.set_u32("NoModify", 1);
    let _ = key.set_u32("NoRepair", 1);

    if let Ok(k) = CURRENT_USER.create(r"Software\EDM") {
        let _ = k.set_string("InstallDir", &target_dir.to_string_lossy());
        let _ = k.set_string("AppPath", &edm_exe_str);
    }

    Ok(())
}

pub fn remove_uninstall_entry() {
    let _ = CURRENT_USER.remove_tree(r"Software\Microsoft\Windows\CurrentVersion\Uninstall\EDM");
    let _ = CURRENT_USER.remove_tree(r"Software\EDM");
}

pub fn register_protocol_handlers(edm_exe: &Path) -> Result<(), String> {
    let edm_exe_str = edm_exe.to_string_lossy().to_string();
    let cmd_str = format!("\"{}\" \"%1\"", edm_exe_str);

    // magnet protocol
    if let Ok(k) = CURRENT_USER.create(r"Software\Classes\magnet") {
        let _ = k.set_string("", "URL:Magnet Protocol");
        let _ = k.set_string("URL Protocol", "");
        if let Ok(cmd_key) = CURRENT_USER.create(r"Software\Classes\magnet\shell\open\command") {
            let _ = cmd_key.set_string("", &cmd_str);
        }
    }

    // .torrent file association
    if let Ok(k) = CURRENT_USER.create(r"Software\Classes\.torrent") {
        let _ = k.set_string("", "EDM.TorrentFile");
    }
    if let Ok(k) = CURRENT_USER.create(r"Software\Classes\EDM.TorrentFile") {
        let _ = k.set_string("", "BitTorrent File");
        if let Ok(cmd_key) = CURRENT_USER.create(r"Software\Classes\EDM.TorrentFile\shell\open\command") {
            let _ = cmd_key.set_string("", &cmd_str);
        }
        if let Ok(ico_key) = CURRENT_USER.create(r"Software\Classes\EDM.TorrentFile\DefaultIcon") {
            let _ = ico_key.set_string("", &format!("\"{}\",0", edm_exe_str));
        }
    }

    Ok(())
}

pub fn remove_protocol_handlers() {
    let _ = CURRENT_USER.remove_tree(r"Software\Classes\magnet");
    let _ = CURRENT_USER.remove_tree(r"Software\Classes\.torrent");
    let _ = CURRENT_USER.remove_tree(r"Software\Classes\EDM.TorrentFile");
}

pub fn register_browser_nmh(target_dir: &Path, nmh_exe: &Path) -> Result<(), String> {
    let nmh_str = nmh_exe.to_string_lossy().to_string();

    let chromium_manifest = serde_json::json!({
        "name": "com.edm.nmh",
        "description": "EDM Native Messaging Host for Chrome, Edge and Brave",
        "path": nmh_str,
        "type": "stdio",
        "allowed_origins": edm_ipc::extensions::allowed_chromium_origins()
    });
    let firefox_manifest = serde_json::json!({
        "name": "com.edm.nmh",
        "description": "EDM Native Messaging Host for Mozilla Firefox",
        "path": nmh_str,
        "type": "stdio",
        "allowed_extensions": edm_ipc::extensions::allowed_firefox_ids()
    });

    let chromium_manifest_path = target_dir.join("com.edm.nmh.json");
    let firefox_manifest_path = target_dir.join("com.edm.nmh.firefox.json");

    if let Ok(content) = serde_json::to_string_pretty(&chromium_manifest) {
        let _ = std::fs::write(&chromium_manifest_path, &content);
    }
    if let Ok(content) = serde_json::to_string_pretty(&firefox_manifest) {
        let _ = std::fs::write(&firefox_manifest_path, &content);
    }

    let chromium_manifest_str = chromium_manifest_path.to_string_lossy().to_string();
    let firefox_manifest_str = firefox_manifest_path.to_string_lossy().to_string();

    let chromium_reg_keys = [
        r"Software\Google\Chrome\NativeMessagingHosts\com.edm.nmh",
        r"Software\Microsoft\Edge\NativeMessagingHosts\com.edm.nmh",
        r"Software\BraveSoftware\Brave-Browser\NativeMessagingHosts\com.edm.nmh",
    ];
    for key in &chromium_reg_keys {
        if let Ok(k) = CURRENT_USER.create(key) {
            let _ = k.set_string("", &chromium_manifest_str);
        }
    }
    if let Ok(k) = CURRENT_USER.create(r"Software\Mozilla\NativeMessagingHosts\com.edm.nmh") {
        let _ = k.set_string("", &firefox_manifest_str);
    }

    Ok(())
}

pub fn remove_browser_nmh() {
    let _ = CURRENT_USER.remove_tree(r"Software\Google\Chrome\NativeMessagingHosts\com.edm.nmh");
    let _ = CURRENT_USER.remove_tree(r"Software\Microsoft\Edge\NativeMessagingHosts\com.edm.nmh");
    let _ = CURRENT_USER.remove_tree(r"Software\BraveSoftware\Brave-Browser\NativeMessagingHosts\com.edm.nmh");
    let _ = CURRENT_USER.remove_tree(r"Software\Mozilla\NativeMessagingHosts\com.edm.nmh");
}

# EDM Architectural Specification: BitTorrent & Magnet Automation Pipeline

**Document ID**: SPEC-2026-BT-01  
**Version**: 1.1.0 (EGM Consensus Approved)  
**Status**: APPROVED FOR SURGICAL EXECUTION  
**Scope**: `packages/engine`, `apps/desktop`, `extension`, Windows Installer & Registry  

---

## 1. Specification Overview & Invariants

This specification defines the automated ingestion, parsing, and execution lifecycle for BitTorrent magnet links, `.torrent` HTTP/HTTPS URLs, and local `.torrent` files in Edge Download Manager (EDM).

### 1.1 Invariants
- **INV-BT-01 (Zero Dead-End Downloads)**: Under no circumstances shall downloading a `.torrent` file terminate with merely saving the `.torrent` file to disk. Every `.torrent` file intercepted or received MUST automatically trigger parsing and begin downloading the torrent's internal files.
- **INV-BT-02 (Cookie & Auth Forwarding)**: When fetching a remote `.torrent` file from private trackers or authenticated portals, the HTTP request must use the user's browser cookies and Referer headers.
- **INV-BT-03 (Single Instance Handoff)**: Opening a `.torrent` file or clicking a `magnet:` link in Windows Explorer or a third-party app when EDM is already running MUST deliver the URI/path to the active EDM instance, focus/unminimize the window, and initiate the download without spawning a separate window or crashing.
- **INV-BT-04 (Bounded Memory & Safe Parsing)**: Local and remote `.torrent` files are strictly capped at 10 MiB to prevent memory exhaustion bombs. Responses returning HTML instead of bencode (private tracker login walls) must fail with a descriptive error.
- **INV-BT-05 (Zip Slip Directory Traversal Guard)**: All target file paths inside multi-file torrents must be strictly normalized and verified to reside inside `task.save_dir`.
- **INV-BT-06 (Cancellation-Aware Magnet Swarm)**: Magnet DHT peer discovery must be wrapped in cancellation-aware futures so users can cancel pending/stalled metadata discovery at any time.
- **INV-BT-07 (Safe Browser Handoff with Fallback)**: The browser extension must obtain a positive WebSocket acknowledgment (`ack.ok`) before canceling native browser downloads.

---

## 2. Component Implementation Details

### 2.1 Engine Layer (`packages/engine`)
#### `packages/engine/src/packs/mod.rs`
- In `classify_target(input: &str)`:
  - Match `.torrent` extension strictly on the clean path (before `?`), rejecting arbitrary `/torrent/` subdirectories:
    ```rust
    let clean_path = lower.split('?').next().unwrap_or("");
    if clean_path.ends_with(".torrent") {
        return ParsedPackTarget {
            pack_id: FeaturePackId::BitTorrentPack,
            clean_url: trimmed.to_string(),
            suggested_filename: None,
            estimated_size: None,
            is_stream: false,
            is_p2p: true,
        };
    }
    ```
  - For local file paths, check if `std::path::Path::new(trimmed).exists()` and ends with `.torrent`.

#### `packages/engine/src/packs/bittorrent.rs`
- In `BitTorrentPack::download_in_process`:
  1. **Remote `.torrent` URLs (`http://` or `https://`)**:
     - Use `reqwest::Client` (with user cookies and referer).
     - Read body stream with `take(10 * 1024 * 1024)` cap.
     - Validate that first byte is `d` (bencode dict) or starts with valid bencode structure. If it starts with `<!doctype` or `<html`, return `Err("Invalid .torrent file: received HTML authentication page instead of bencode data".into())`.
     - Pass buffer to `librqbit::AddTorrent::from_bytes(data)`.
  2. **Local `.torrent` File Paths**:
     - Check file existence and size <= 10 MiB.
     - Read file bytes and pass to `AddTorrent::from_bytes(data)`.
  3. **Direct Magnet URIs (`magnet:?xt=...`)**:
     - Parse magnet query parameters to obtain preliminary name and trackers.
     - Call `AddTorrent::from_url(&task.url)`.
     - Wrap `session.add_torrent(...)` with `tokio::select!` checking `task.cancelled` periodically.
  4. **Deferred Disk Space Check**:
     - As soon as `handle.stats().total_bytes > 0` becomes known, call `crate::security::check_disk_space(&task.save_dir, stats.total_bytes)`.
  5. **Telemetry & Swarm Loop**:
     - Broadcast progress, download speed, active peer count, and worker metrics every 500ms.
     - Gracefully close `session` when download completes or is cancelled.

### 2.2 Security & Dispatch Layer (`apps/desktop/src-tauri`)
#### `apps/desktop/src-tauri/src/os/files.rs`
- In `validate_url_security`:
  - Allow Windows local file paths ending in `.torrent`:
    ```rust
    let path = std::path::Path::new(trimmed);
    if path.is_absolute() && path.extension().map_or(false, |e| e.eq_ignore_ascii_case("torrent")) {
        return Ok(url::Url::from_file_path(path).unwrap_or_else(|_| url::Url::parse("https://local.torrent/").unwrap()));
    }
    ```

#### `apps/desktop/src-tauri/src/main.rs`
- Extract a unified helper `handle_cli_target(app: &tauri::AppHandle, target: &str)`:
  - If `target` starts with `magnet:`, `http://`, `https://`, or is a valid `.torrent` path:
    - Bring window to foreground: `window.show(); window.unminimize(); window.set_focus();`
    - Read default download dir from settings store.
    - Invoke `execute_download(...)`.
- Call this helper in both `setup` hook (for cold boot `std::env::args()`) and `tauri_plugin_single_instance` (for warm second-instance wakeups).

### 2.3 Windows Registry & NSIS Registration
#### `apps/desktop/src-tauri/windows/hooks.nsh` & `src/os/registry.rs`
- Register `magnet` protocol handler:
  ```nsis
  WriteRegStr HKCU "Software\Classes\magnet" "" "URL:BitTorrent Magnet URL"
  WriteRegStr HKCU "Software\Classes\magnet" "URL Protocol" ""
  WriteRegStr HKCU "Software\Classes\magnet\DefaultIcon" "" "$INSTDIR\EDM.exe,0"
  WriteRegStr HKCU "Software\Classes\magnet\shell\open\command" "" '"$INSTDIR\EDM.exe" "%1"'
  ```
- Register `.torrent` file association:
  ```nsis
  WriteRegStr HKCU "Software\Classes\.torrent" "" "EDM.TorrentFile"
  WriteRegStr HKCU "Software\Classes\EDM.TorrentFile" "" "BitTorrent Document"
  WriteRegStr HKCU "Software\Classes\EDM.TorrentFile\DefaultIcon" "" "$INSTDIR\EDM.exe,0"
  WriteRegStr HKCU "Software\Classes\EDM.TorrentFile\shell\open\command" "" '"$INSTDIR\EDM.exe" "%1"'
  ```

### 2.4 Browser Extension (`extension`)
#### `extension/src/background.ts`
- In `chrome.downloads.onDeterminingFilename`:
  - If filename ends with `.torrent` or mime type is `application/x-bittorrent`:
    - Send `create_task` to EDM via WebSocket including active cookies and referrer.
    - Wait for `ack`: if `ack?.ok`, cancel the native browser download (`chrome.downloads.cancel(item.id)`). If offline/unacknowledged, let browser download proceed naturally.

### 2.5 Desktop Frontend (`apps/desktop`)
#### `apps/desktop/src/components/AddDownloadModal.tsx`
- Add a `.torrent` file selection button using Tauri's native dialog or file input, populating the URL input field with the chosen file path.
#### `apps/desktop/src/App.tsx`
- Listen for Tauri v2 window drag-drop events (`getCurrentWebviewWindow().onDragDropEvent(...)`) to auto-open `AddDownloadModal` or immediately start download when a `.torrent` file is dropped.


# Changelog

All notable changes to Edge Download Manager (EDM) are documented in this file.

## [v0.0.22] - 2026-11-20

### ⚡ Hotfix: Actions Column Header Symmetry & Alignment

#### 1. Table Header Actions Alignment
- **Root Cause**: While row cells align action buttons to the right (`justify-end`, `pr-3 sm:pr-4`) taking up ~130px, setting `text-left` on the outer `<th>` caused the "Actions" title to sit too far to the left of the column, floating disconnected over empty space.
- **Symmetric Alignment**: Wrapped the header label in a right-aligned container with a matching `w-[130px] text-left` slot:
  ```tsx
  <th className="py-2 px-3 pr-3 sm:pr-4 w-48 min-w-[188px] whitespace-nowrap">
    <div className="flex items-center justify-end">
      <div className="w-[130px] text-left select-none">Actions</div>
    </div>
  </th>
  ```
  The text "Actions" now starts exactly aligned with the left edge of the first action button across all rows and resolutions.

## [v0.0.21] - 2026-11-20

### ⚡ Critical Hotfix & Architecture: Dedicated Lifecycle Engine, Autonomous Zero-Click Update, and LIFO Download Queue

#### 1. Dedicated Backend Lifecycle Subsystem (`apps/desktop/src-tauri/src/lifecycle/`)
- **Architectural Isolation**: Extracted 700+ lines of fragmented lifecycle logic out of `commands/system.rs` and `updater.rs` into a modular subsystem:
  - `lifecycle/types.rs`: Clean schema definitions for `InstallOptions`, `RepairPlan`, `RepairReport`, `ExistingInstallationInfo`, `UpdateCheckResult`, and `RuntimeMode`.
  - `lifecycle/platform/windows.rs`: Native Win32 COM (`IShellLinkW` / `IPersistFile`) shortcut generation (completely replacing child `powershell.exe` calls with zero console flashing), pure Win32 registry manipulation for NMH, autostart, protocols, and Toolhelp32 process & file lock checks.
  - `lifecycle/install.rs`: Native installation engine with directory validation, binary co-location, and NMH registration.
  - `lifecycle/repair.rs`: Targeted diagnostics & repair for browser NMH, COM shortcuts, and binary integrity with zero user data loss.
  - `lifecycle/update.rs`: Polling releases, secure asset streaming, staging to `%LOCALAPPDATA%\EDM\staging\`, pre-flight file lock validation, and detached handoff without in-place deadlocks.
  - `lifecycle/uninstall.rs`: Native uninstallation (NMH, protocols, shortcuts, user data purge vs keep settings, detached cleanup).
  - `lifecycle/manager.rs` & `lifecycle/mod.rs`: Runtime mode dispatcher (`Normal`, `Setup`, `Repair`, `Update`, `Uninstall`), window sizing (680x440), and background service gating.

#### 2. Dedicated Frontend Lifecycle Directory & Autonomous Zero-Click Update UI
- **Frameless Lifecycle Suite (`apps/desktop/src/lifecycle/`)**:
  - `wizards/UpdateWizard.tsx`: Modern frameless dark glassmorphic 680x440 update UI. Autonomous zero-click execution: streams OTA update payload, listens to live progress, displays real-time transfer rate and ETA telemetry, release notes, staging status pipeline, and auto-restart countdown. Replaces legacy Win32 NSIS dialogs completely.
  - `wizards/InstallWizard.tsx`, `wizards/RepairWizard.tsx`, `wizards/UninstallWizard.tsx`: Relocated under `lifecycle/wizards/` with backwards-compatible re-exports in `components/`.
  - `LifecycleApp.tsx`: Unified shell routing `#setup`, `#repair`, `#update`, `#uninstall`.

#### 3. NSIS Modern UI Delegation (Zero Legacy NSIS Dialogs)
- **Root Cause of Legacy NSIS Window**: Passing `/P` (passive mode) set `$PassiveMode = 1` in `installer.nsi`, which skipped the modern UI hook (`$PassiveMode <> 1`) and fell back to the 1995-era Win32 `INSTFILES` extract dialog.
- **Modern Delegation**: Updated `installer.nsi` so `/UPDATE` or update hand-off launches `"$PLUGINSDIR\${MAINBINARYNAME}.exe" --update` via the modern frameless UI instead of falling through to `MUI_PAGE_INSTFILES`.

#### 4. LIFO Download Stacking Order & Actions Column Left-Alignment
- **LIFO Download Order**: Added `createdAt: number` timestamp to `DownloadTask`. Updated `App.tsx` sort comparator for `dateAdded` to use `(b.createdAt - a.createdAt)`, ensuring the newest download is placed at row 1 (file 1 becomes 2nd when file 2 is added).
- **Actions Column Alignment**: Updated table header `<th className="text-right">` to `<th className="text-left pl-3 sm:pl-4">` for symmetric column alignment.

## [v0.0.19] - 2026-11-19

### ⚡ Hotfix: Extension Queued Download State & Parity Alignment

#### 1. Extension Popup Queued State Rendering
- **Root Cause**: In `extension/src/popup.ts` (and transpiled `popup.js`), `renderDownloadItem()` processed task status through an `if / else if / else` ladder covering `isDownloading`, `isCompleted`, `isPaused`, and `link_expired`. Because `queued` and `connecting` states were missing from the ladder, queued tasks fell through to the fallback `else` branch, which erroneously rendered `<span style="color: #F87171;">Failed</span>` with zero size progress despite the task waiting legitimately in the backend queue.
- **Explicit Queued Representation**:
  - Added SVG clock icon and explicit `isQueued` evaluation for states `"queued"` and `"connecting"`.
  - Rendered a blue accent badge with clock icon, formatted file size, and `"Queued"` status indicator.
  - Added primary `"Start Download Now"` resume action button and cancel button for queued cards, matching desktop app parity.
  - Added `.download-card.card-queued` styling in `popup.css`.
  - Updated active task tab count to include queued and connecting downloads matching badge and header counters.

## [v0.0.18] - 2026-11-15

### ⚡ Critical Hotfix: In-Place Auto-Update Deadlock Fix, Zero-Console Process Lifecycle & Non-Elevated Relaunch

#### 1. In-Place Auto-Update Deadlock Fix
- **Root Cause**: `main.rs` unconditionally intercepted Tauri's `RunEvent::ExitRequested` event and called `api.prevent_exit()` to implement close-to-tray. When `updater.rs` initiated an update and invoked `trigger_graceful_quit()`, Tauri's exit was suppressed! The process lingered holding PE file locks on `EDM.exe` for 5 seconds until an uncoordinated fallback thread killed it. NSIS in passive mode (`/P`) started within 150ms and attempted to overwrite `EDM.exe`, which failed with `ERROR_SHARING_VIOLATION`.
- **Atomic Quit Protocol**: Introduced `pub static IS_QUITTING: AtomicBool`. Set to `true` during `trigger_graceful_quit()`. `main.rs` checks `IS_QUITTING` in `RunEvent::ExitRequested` and immediately permits clean shutdown. Reduced fallback delay from 5s to 1s.
- **Canonical Binary Target**: Defined `[[bin]] name = "EDM" path = "src/main.rs"` in desktop `Cargo.toml`. Tauri and Cargo now canonically produce `EDM.exe` as the primary executable.

#### 2. Complete Elimination of Console Flickering (Zero-Console Across App)
- **Root Cause**: During install, update, reinstall, and uninstall, console windows flickered 2–3 times due to:
  1. `installer.nsi` and `hooks.nsh` invoking `taskkill.exe` via `nsExec::Exec` 3 consecutive times. On Windows 10/11 with modern Windows Terminal, CUI binaries flash open/close.
  2. `system.rs` invoking `Command::new("powershell")` 11 times (process termination, shortcut creation, cleanup) without Win32 `CREATE_NO_WINDOW` (`0x08000000`).
- **Zero-Console Invariant**:
  - Attached `.creation_flags(CREATE_NO_WINDOW)` to all 11 `std::process::Command::new("powershell")` invocations in `system.rs`.
  - Consolidated multiple sequential `taskkill` calls in `installer.nsi` and `hooks.nsh` into single compound invocations: `taskkill /F /IM EDM.exe /IM edm-desktop-app.exe /IM edm-nmh.exe /T`.

#### 3. Standard Non-Elevated Relaunch via `ExecShell`
- **Root Cause**: In `installer.nsi`, `.onInstSuccess` and `RunMainBinary` invoked `nsis_tauri_utils::RunAsUser "$INSTDIR\${MAINBINARYNAME}.exe"`. On standard `currentUser` non-elevated installations, `RunAsUser` failed with `ERROR_PRIVILEGE_NOT_HELD` (Win32 1314) because standard tokens cannot duplicate shell tokens.
- **Conditional Relaunch**: Implemented conditional compile check: for `currentUser` installs, use `ExecShell "" "$INSTDIR\${MAINBINARYNAME}.exe"` to relaunch directly without elevation failure.

## [v0.0.17] - 2026-11-15

### ⚡ Hotfix: Centralized Queue Manager, Compact Toast UI, and BitTorrent Seeding & Telemetry Engine

#### 1. PART A: Centralized FIFO Queue Manager & Concurrency Limit
- **Root Cause**: Adding a download when `active >= max_active` previously allowed optimistic frontend state transition to `"downloading"` while backend tasks lacked active task registration, causing telemetry polling failures and marking downloads as Failed. Furthermore, pausing, cancelling, or updating settings never invoked queue slot refills.
- **Atomic Queue Scheduling**: Added centralized `try_start_next_queued` in `download_manager.rs`. Any download added above concurrency limits atomically receives `DownloadState::Queued` without failure.
- **Event-Driven Drain**: Automatically triggers next queued item in FIFO creation order on task completion, pause, cancellation, removal, and runtime concurrency limit adjustments in Settings.

#### 2. PART B: Compact Glassmorphic Update Toast
- **Root Cause**: The update notification container was constrained by overly aggressive truncation classes, and a redundant green badge occupied horizontal layout space.
- **Redesigned Notification Toast**: Removed the green badge completely. Rendered a sleek, horizontally compact glassmorphic pill showing full untruncated version text (`EDM v0.0.17`), subtle `Release notes ↗` link, primary `Update Now` button, and quick dismiss `X`.

#### 3. PART C: BitTorrent Seeding, Swarm View, Resume Hashing & Telemetry
- **C1 - Seeding & Incoming Listener**: Configured `librqbit::ListenerOptions` with `enable_upnp_port_forwarding = true` and `0.0.0.0:0` TCP listener. Real-time upload throughput is tracked and reported across telemetry ticks.
- **C2 - Dedicated Torrent Swarm View**: Replaced HTTP 206 byte-range table with a dedicated Torrent Swarm view displaying connected peers, share ratio, uploaded bytes, swarm health, and peer piece exchanges.
- **C3 - Zero Fake Speed Spikes on Resume**: During piece hash verification (`is_initializing`), reported state is `"Verifying"`, network transfer rate is clamped to `0 B/s`, and ETA displays `"Verifying existing data on disk..."`, ensuring disk hashing bytes are never counted toward network download metrics.
- **C4 - Worker Speed Sum & Segment Offsets**: Synced worker speeds proportionally so their sum matches the header download rate. Persisted `SegmentRecord`s via `save_segment_positions` on pause/checkpoint so HTTP multi-stream downloads resume from their exact byte offsets without re-splitting.
- **C5 - Real Thread Tracking**: Updated bottom status bar to report live active connections (reporting 0 when paused, queued, or completed).
- **C6 - Smoothed ETA & Elapsed Ticker**: Applied exponential moving average (alpha=0.1) with 3-sample warmup gate for stable ETA calculations, and ensured monotonic elapsed time formatting.
- **C7 - Pre-Populated Speed Chart**: Pre-seeds 60-second rolling telemetry curve when details drawer opens to eliminate initial flatline rendering.

## [v0.0.16] - 2026-11-15

### ⚡ Critical Hotfix: Zero-Console Updater Engine, Detached Shell Execution & Sibling Cleanup

#### 1. Zero Console Window Flashing (No `cmd.exe`)
- Eliminated `cmd.exe` intermediary script and timeout batch execution entirely from the updater pipeline.
- Implemented direct Windows Shell execution via `ShellExecuteW` (with `"open"` and automatic elevation fallback `"runas"`) as well as a detached fallback using `CREATE_BREAKAWAY_FROM_JOB | CREATE_NEW_PROCESS_GROUP`.
- Invokes installer with NSIS passive mode flags `/P /R` ensuring visible progress feedback and clean post-install relaunch without a terminal popup.

#### 2. Persistent Update Diagnostics Logging
- Added persistent diagnostic logging to `%LOCALAPPDATA%\EDM\logs\update.log`.
- Tracks installer package download validation, size verification, ShellExecuteW return handles, process termination results, and error traces.

#### 3. Complete Pre-Install Process Termination
- Integrated atomic sibling process cleanup terminating `edm-nmh.exe`, `edm-desktop-app.exe`, and other `EDM.exe` instances while protecting the current host PID.
- Waits up to 5 seconds to ensure all file locks on `%LOCALAPPDATA%\EDM` binaries are completely released before installer execution.

#### 4. Resilient Failure Recovery & UI Controls
- If installer launch fails, EDM remains open rather than abruptly terminating.
- Added "Open folder" and "Retry" action buttons in both the floating in-app update banner and Settings About view, backed by native `open_installer_folder` IPC.

## [v0.0.15] - 2026-11-15

### 🛠️ Dedicated Repair Mode, YouTube Completion Mark, BitTorrent Upload Telemetry, and Policy Guardrails

#### 1. Dedicated Frameless Reinstall & Repair Mode (`#repair` / `--repair`)
- **Independent Diagnostic Chassis**: Created dedicated `RepairWizard.tsx` rendered in a fixed 680×440 frameless dark glassmorphic window matching EDM's design system, completely detached from the standard installer and OTA update flows.
- **Targeted Subsystem Repair**: Provides checkbox-driven repair actions:
  - Re-registers browser integration manifests and registry keys (`com.edm.nmh`) for Microsoft Edge, Google Chrome, Brave, and Mozilla Firefox.
  - Restores missing Windows shell Desktop and Start Menu shortcuts (`EDM.lnk`).
  - Verifies and synchronizes core application binaries (`EDM.exe`, `edm-desktop-app.exe`, `edm-nmh.exe`).
- **Non-Destructive Guarantee**: Explicit badge confirming zero data loss ("Preserves all downloads, queues, and user settings").
- **Live Progress & Completion**: Diagnostic progress bar and success summary with a one-click "Launch EDM" button.

#### 2. YouTube Completion Mark in Extension Popup
- **Atomic State Synchronization**: Resolved race condition where late throttled progress packets in Tokio background tasks could overwrite `"completed"` state with `"downloading"`.
- **Defensive State Invariant**: Guarded `update_task_progress` in `ws/manager.rs` to prevent completed or failed tasks from reverting back to downloading.
- **Immediate Finished State in Popup**: Updated `extension/src/popup.ts` to detect 100% completion (size done >= size total at 0 B/s), immediately displaying the `✓ Finished` mark in the "Done" tab with Open File/Folder actions and removing pause/cancel controls.

#### 3. BitTorrent Live Upload Telemetry & Elapsed / Time Left Fix
- **Live Upload Throughput**: Added `upload_throughput_bps` to `AdaptiveTelemetry` and populated live upload rate from `librqbit` (`stats.live.upload_speed`) across telemetry ticks.
- **Speed Chart Dual-Series Display**: Bound upload history in `SpeedChartInteractive.tsx` and `DetailsInspectorDrawer.tsx` so the status box displays live upload throughput alongside peak rates.
- **Accurate Elapsed Time & ETA**: Computed real elapsed time from task start in `App.tsx` and removed restrictive checks so "Elapsed" and "Time left" render dynamically.

#### 4. File Collision & Conflict Policy Enforcement
- **Settings-Driven Handling**: Replaced unconditional duplicate renaming with strict adherence to `settings.storage.filenameConflictPolicy`:
  - `overwrite`: Replaces existing target file directly.
  - `rename`: Automatically resolves unique name with numerical counter (`name (1).ext`).
  - `skip`: Gracefully aborts duplicate download with explicit status message.
  - `ask`: Displays a native Windows conflict resolution dialog (`MessageBoxW`) allowing the user to choose Overwrite, Auto-Rename, or Cancel.

#### 5. Multi-Stream Acceleration & Concurrency Limit Queueing
- **Dynamic Concurrency Enforcement**: Enforced `settings.downloads.maxActiveDownloads`. When active downloads reach the configured limit, subsequent downloads are cleanly inserted as `Queued` in SQLite and WebSocket manager.
- **Automatic Queue Drain**: Upon task completion or cancellation, or when `maxActiveDownloads` is increased in Settings, the next queued task is automatically started.
- **Segmentation Strategy Rules**: Enforced `segmentedDownloads` modes (`disabled` locks connections to 1, `aggressive` expands connections up to 16+).

#### 6. System Toast Alert & Audio Chimes
- **Direct Notification Dispatch**: Fixed completion trigger condition in `App.tsx` so notification sound and Windows toast alerts fire immediately when a task finishes.
- **Native Backend Fallback**: Added direct `send_system_notification_internal` invocation in `download_manager.rs` on task completion as a fail-safe.

#### 7. Extension Rebranding & OTA Label Cleanup
- **Extension Name Clean**: Renamed browser extension from `"EDM — Edge Download Manager"` to strictly `"EDM"` across all Chromium and Firefox manifests.
- **Pro Badge & OTA Removal**: Removed `"PRO "` prefix from version badge in Settings About section (`v0.0.15`) and stripped `"OTA"` references across UI labels.

## [v0.0.14] - 2026-11-15

### 🚀 Isolated Handoff Architecture, Immortal Socket Resilience, Version Parity & Update UX

#### 1. Isolated Communication Architecture (`core/handoff/`)
- **Hermetic Subsystem Boundary**: Established dedicated `core/handoff/` directory separating Extension ↔ Bridge (NMH) ↔ Desktop Application communication from UI and download engine blast radiuses.
- **Dual-Language Contract Types**: Synchronized TypeScript interfaces (`contract/types.ts`) and Rust structures (`contract/types.rs`) with numeric semver parsing, constants, and whitelisted extensions.
- **Resilient Host Listener & Token Auth**: Implemented loopback TCP/WS server with constant-time token verification (`host/auth.rs`) against `%APPDATA%\EDM\token.txt`.
- **Standalone Test Harness**: Created headless test harness (`test-harness/run_harness.ps1`, `test-harness/mock_extension.mjs`) verifying NMH diagnostics, process wake, and HTTP token endpoints in CI and local dev without launching WebView2.

#### 2. Immortal Socket Resilience & Zombie Self-Healing
- **Immortal Accept Loop**: Converted Tokio TCP listener from `while let Ok` to an immortal `loop { match ... }`, eliminating socket drops on transient Windows `WSAECONNRESET` / `ECONNABORTED` loopback probes.
- **Asynchronous Bind Retry Loop**: Replaced one-shot `TcpListener::bind` with a 250ms backoff loop, preventing permanent listener death when port 14370 is temporarily held in `TIME_WAIT` during restarts or updates.
- **Setup & Uninstall Wizard Port Isolation**: Excluded `--setup` and `--uninstall` installer modes from binding port 14370, eliminating port lockouts on the main application.
- **NMH Zombie Auto-Recovery**: If port 14370 does not respond within 4 seconds during wake, `edm-nmh` automatically terminates stale zombie `EDM.exe` background processes and re-launches a fresh instance.

#### 3. Single Source of Truth & Runtime Version Parity
- **Unified Version Source**: Enforced `tauri.conf.json` as the single source of truth across all 7 package targets (`Cargo.toml`, `package.json`, `manifest.json`, etc.), managed via `scripts/bump-version.mjs`.
- **Zero Hardcoded Runtime Versions**: Dynamic version reads via `@tauri-apps/api/app` `getVersion()` in UI, `CARGO_PKG_VERSION` in Rust, and `chrome.runtime.getManifest().version` in the browser extension.
- **Post-Update Verification**: Verified active running build against pending update targets on first launch to ensure accurate status reporting.

#### 4. Browser Extension Bundling & Sync Logic
- **Esbuild Bundle Mode**: Enabled `bundle: true` in `extension/build.js`, eliminating `Cannot use import statement outside a module` syntax errors in classic Chromium MV3 service workers and popup scripts.
- **Numerical Semver Version Comparator**: Replaced string comparisons with `compareVersions` across extension and desktop components.
- **Accurate Sync Statuses**: Introduced distinct UI states: "Up to Date" (green), "Update Available" (orange), and "Ahead of App" (blue), preventing false update notices on downgrades.

#### 5. Streamlined Update UI & Custom Installer Flow
- **Glassmorphic In-App Banner**: Redesigned update notification with clean one-line summary, external release notes link, and persistent dismissal tracking across sessions.
- **Streamlined Update Wizard**: Custom installer wizard automatically detects existing installations, omitting redundant folder selection and desktop shortcut options for a seamless one-click update experience.

#### 6. Architectural Invariant Gates
- **Pre-Flight Invariant Verification**: Added `scripts/verify-invariants.ps1` enforcing 6 critical gates (non-destructive payloads, bidirectional co-location, NMH `%LOCALAPPDATA%` resolution, WebView2Loader integrity, and version alignment).
- **Automated Verification Suite**: Added `tests/v0_14_verification_suite.test.mjs` with 10 passing end-to-end integration tests.

## [v0.0.13] - 2026-11-13

### ⚡ Dynamic System Folders, Pictures Category, Custom Setup & Uninstaller Wizards, and Windows Settings Parity

#### 1. Custom Frameless Setup, Update & Uninstaller Wizards
- **Modern Setup & Update UI**: Implemented interactive 680×440 frameless dark glassmorphic UI (`InstallerWizard.tsx`). NSIS directly delegates fresh installations, reinstalls, and updates to the modern UI, with automatic detection of existing installs and "Update Now" flow.
- **Custom Modern Uninstaller (`UninstallerWizard.tsx`)**: Built frameless 680×440 uninstaller window triggered by `--uninstall` from Windows Settings. Displays EDM branding, removal options (with optional user data purge), progress visualization, and safely cleans all components.
- **Process Lock Prevention & Detached Self-Deletion**: Terminated background `EDM.exe` and `edm-nmh.exe` instances prior to updates/reinstalls/uninstalls, eliminating `os error 5` / `Access is denied` sharing violations. Spawns detached background script to delete application binaries upon process exit.
- **Start Menu Shortcut Persistence**: Added automatic creation of `%APPDATA%\Microsoft\Windows\Start Menu\Programs\EDM.lnk` ensuring EDM appears in the Windows Start menu and search section, with full cleanup on uninstall.
- **Windows Settings App Listing Metadata**: Populated uninstall registry with `DisplayVersion: "0.0.13"`, `Publisher: "bytebldr"`, and `InstallDate: "YYYYMMDD"`, ensuring Windows Settings displays `0.0.13 | bytebldr | 13-11-2026`.

#### 2. Dynamic User Directories & Dedicated Subfolders
- **Dynamic Known Folders Resolution**: Exposed `get_system_user_dirs` in the Rust backend (`os/files.rs` / `commands/system.rs`), querying Windows Known Folders dynamically rather than relying on hardcoded system paths.
- **Dedicated Category Folder Hierarchy**:
  - `Documents` → `%USERPROFILE%\Documents\EDM`
  - `Music / Audio` → `%USERPROFILE%\Music\EDM`
  - `Pictures` → `%USERPROFILE%\Pictures\EDM`
  - `Videos` → `%USERPROFILE%\Videos\EDM`
  - `Programs` → `%USERPROFILE%\Downloads\Programs`
  - `Archives` → `%USERPROFILE%\Downloads\Archives`
- **Zero Empty Folders Invariant (Just-In-Time Allocation)**: Subfolders are never created in advance. Folders are created on disk strictly when a download matching that category starts writing bytes (`FileWriter::new` and `PipelineWriter::new`), keeping the user's filesystem 100% free of ghost folders.
- **Added 6th Category (Pictures)**: Integrated dedicated Pictures category with `Image` icon and expanded file classifications (`jpg, png, webp, avif, svg, bmp, ico, tiff, psd, ai, raw, cr2, nef, heic, dng`, etc.).
- **Per-Category Folder Browse Dialog**: Added a "Browse..." button (`FolderOpen` icon) next to every category directory in `SettingsPage.tsx` using native Tauri file dialogs.

#### 2. Segregated Notification Architecture
- **Download Completed (System-Only Toast)**: Implemented `send_system_notification` using native Windows PowerShell WinRT XML toasts with `AppUserModelID = "com.edm.desktop"`, audio chime (`ms-winsoundevent:Notification.Default`), and an interactive "Open Folder" action button. Zero in-app toast spam.
- **Update Detected (In-App Center-Down Banner)**: Floating chassis pill anchored center-down right above the bottom status bar (`fixed z-50 bottom-14 left-1/2 -translate-x-1/2`) with Astryx glassmorphism styling, shiny version badge, `Update Now` primary button, and dismiss action. Never fires intrusive OS notifications.

#### 3. Feature Packs UI Streamlining & yt-dlp Update
- **Completely Removed Filter Box**: Stripped the awkward filter pill container and category filter buttons, presenting all 9 feature pack cards cleanly and directly in the compact grid.
- **yt-dlp Engine Update**: Bumped `yt-dlp` package version definition to `2026.10.26`.

#### 4. Custom Windows Setup Installer Branding
- **Custom Modern Bitmaps**: Generated high-fidelity 24bpp RGB bitmaps (`nsis-sidebar.bmp` and `nsis-header.bmp`) derived from the modern obsidian/neon-violet mockup design.
- **Branded Setup Wizard**: Updated `installer.nsi` with custom `BrandingText` (*"EDM — Fast, Intelligent Native Download Accelerator"*), customized Welcome and Finish page copy, and seamless in-place upgrade handling.

#### 5. Extension Parity with Astryx Design Tokens
- **Astryx Specular Styling**: Upgraded `popup.css` with surface specular highlights, smooth active state transitions, and `.tile-pictures` category badge coloring.
- **Classifier Synchronization**: Updated extension `popup.ts` to recognize the `pictures` category and classify all expanded extensions in real time.


## [v0.0.12] - 2026-11-10

### 🛡️ Hotfix: BitTorrent Swarm Resumption, Overwrite Guard & YouTube Clean Protocol

#### 1. BitTorrent Overwrite & Resume Support
- **Unconditional File Overwrite Flag**: Set `AddTorrentOptions { overwrite: true, ..Default::default() }` in `librqbit` in-process engine so re-downloading or resuming existing files on disk verifies local piece hashes and seeds/resumes rather than failing with `File exists`.
- **Fail-Fast Swarm Error State Interception**: Handled `librqbit::TorrentStatsState::Error` immediately, eliminating 0-bps infinite loops.
- **Verification Phase Telemetry**: Implemented `TorrentStatsState::Initializing` handling with `phase: "Verifying"` and `"Verifying existing pieces on disk..."`, refreshing UI progress timers to prevent false stalled states.
- **Tier-1 Public Tracker Injection**: Enriched magnet links with curated Tier-1 public trackers (`tracker.opentrackr.org`, `open.demonii.com`, etc.) for instantaneous swarm discovery.
- **Recursive Multi-File Payload Extraction**: Added `find_largest_file_recursive` to resolve the primary target file across arbitrary directory depth.

#### 2. YouTube Robust Client & Templating Pipeline
- **Clean Default Client Negotiation**: Removed `--extractor-args "youtube:player_client=ios,web,default"` to prevent iOS SABR/PO-Token 403 lockouts; yt-dlp now leverages Node.js challenge solving with native web/Samsung/VisionOS player APIs.
- **Granular Progress Streaming**: Removed `--http-chunk-size 10M` to eliminate false 10-second stalled trips.
- **Collision-Free Dynamic Templating**: Automatically converts generic filenames (`"watch"`, `"watch.mp4"`, `"video"`, `"download"`, `"download.bin"`) to `"%(title)s.%(ext)s"`, preventing multiple YouTube downloads from colliding into `watch.mp4`.
- **Force Overwrite on Re-Download**: Added `--force-overwrites` to cleanly overwrite incomplete or existing files on explicit duplicate confirmation.
- **Captured Pre-Existing Target**: Handled `[download] <path> has already been downloaded` with absolute path normalization and immediate 100% completion telemetry.
- **Fallback User-Agent & Concurrency**: Passed Android VR User-Agent and single-connection concurrency to in-process `YouTubeExtractor` fallback to avoid Google Video CDN 403 rejections.

#### 3. Browser Extension: Dynamic YouTube Quality Selector
- **CSP-Compliant Stream Inspection**: Replaced CSP-blocked inline script injection with direct DOM script extraction and a MAIN-world companion script (`yt-main.ts`).
- **Dynamic Per-Video Resolution Selection**: The dropdown dynamically presents only the resolutions actually available for the current playing video (8K, 4K, 1440p, 1080p, 720p, 480p, 360p, 240p, 144p) with exact file sizes and framerate badges (e.g. `60fps`), eliminating the bug where only "Best Available Quality" appeared.
- **Fragment-Based Engine Routing**: Downloads cleanly route chosen resolutions (`#2160p`, `#1080p`, `#720p`, etc.) directly into EDM's native video pack.

## [v0.0.11] - 2026-11-10

### 🚀 Seamless In-Place Upgrades, Custom Installer Branding & Compact Settings UI

#### 1. Seamless In-Place Upgrades in NSIS Installer
- **Eliminated Uninstall Prompt**: Intercepted `PageReinstall` in the NSIS installer template (`installer.nsi`) to bypass the prompt requiring users to uninstall older versions before upgrading.
- **Data & Configuration Preservation**: Performs in-place binary overwrite in `$INSTDIR` while fully preserving `%LOCALAPPDATA%\EDM`, download history, and user settings.

#### 2. Custom NSIS Installer Branding
- **Branded Setup Executable**: Configured custom installer icon (`icon.ico`) and embedded high-resolution 24-bit bitmaps for installer header (`nsis-header.bmp`) and sidebar wizard (`nsis-sidebar.bmp`), replacing default Nullsoft graphics.

#### 3. Compact Settings Feature Pack Cards
- **High-Density Card Grid**: Redesigned Feature Packs in `SettingsPage.tsx` into a responsive, compact 3-column card grid showing pack icon, display name, and version badge, reducing vertical footprint and visual clutter.

#### 4. Automated BitTorrent & Magnet Link Swarm Pipeline
- **Zero Dead-End .torrent Downloads**: When clicking a `.torrent` link or opening a file, EDM automatically fetches and parses the bencode metadata in-memory and launches the in-process `librqbit` swarm to download the content rather than terminating with an inert `.torrent` file.
- **Direct Magnet Link Handling**: Fully automated `magnet:?xt=...` link parsing, DHT peer discovery, cancellation-aware resolution, and deferred disk space verification.
- **Windows Shell & Protocol Integration**: Configured `magnet:` protocol handler and `.torrent` file association (`EDM.TorrentFile`) in NSIS installer hooks and runtime registry, routing external launches to the running EDM instance with automatic window focus.
- **Desktop UI Ingestion**: Added a `.torrent` file picker dialog in the Add Download modal and native window drag-and-drop ingestion.

#### 5. Intelligent Upfront & Adaptive Wrapper Subsystem (`adaptive_wrapper`)
- **Universal 9-Pack Pre-Flight & Post-Flight Architecture**: Dedicated module in `packages/engine/src/adaptive_wrapper/` wrapping all 9 protocol engines (HTTP, BitTorrent, FTP, eD2k, M3U8, FFmpeg, yt-dlp, GitHub, Hugging Face).
- **Sub-600ms Speculative Latency Racing**: Races origin and mirror CDN endpoints (GitHub raw/releases, Hugging Face models) in parallel with strict credential isolation and automatic failover.
- **Dual-Stream Parallel Pipelining**: Simultaneously downloads high-res video and audio tracks across 16 parallel workers directly in the Adaptive Engine, followed by zero-copy in-process FFmpeg container stitching (`-c copy`).
- **ProcessGuard & Async Stderr Draining**: Eliminates OS pipe buffer deadlocks during media remuxing by draining stderr concurrently with stdout while streaming real-time heartbeats to NetPulse.
- **Third-Party Feature Pack Attributions**: Added comprehensive `THIRD_PARTY_CREDITS.md` documenting upstream engines (`librqbit`, `yt-dlp`, `FFmpeg`, `RustCrypto`, `ghproxy`, `HF-Mirror`).

## [v0.0.9] - 2026-11-08

### 🚀 Dynamic Media Quality Probing, Modular Architecture & Release Hardening

#### 1. Dynamic YouTube Stream & Quality Discovery
- **Zero-Hardcoding Format Resolution**: Replaced static quality options with dynamic runtime format probing via `packages/engine/src/packs/ytdlp.rs` and `probe_video_url` IPC command.
- **Genuine Available Quality Extraction**: Accurately discovers available video resolutions (4K/2160p, 1440p/2K, 1080p60, 720p, 480p, 360p) and audio streams for each specific video, preventing low-quality fallback when higher resolutions exist.
- **In-Player Native Pill Integration**: Extends the native YouTube quick download pill next to the Subscribe button to render available video resolutions, codec badges (VP9/AV1), and audio-only extraction formats.

#### 2. Workspace Modularization & Architecture Refactoring
- **Modern Package Directory Layout**: Refactored monolithic directory naming into clean, modular packages:
  - `packages/engine` (`edm-core`): Adaptive multi-stream chunk downloader, range set math, and integrity verification.
  - `packages/browser-host` (`edm-nmh`): Chrome/Gecko Native Messaging Host binary with stdio framing and auto-wake supervisor.
  - `packages/ipc` (`edm-ipc`): Extension IDs, WebSocket message schemas, and cross-process data types.
  - `packages/store` (`edm-store`): SQLite persistent storage layer with batched writes and migrations.
  - `packages/cli` (`edm-cli`): Headless command-line download utility.
  - `packages/mock-server` (`edm-testsrv`): In-process mock HTTP testing server for chaos diagnostics.
  - `apps/desktop`: Tauri v2 desktop user interface with React 19 and Tailwind CSS.
  - `extension`: Manifest V3 browser extension for Chromium and Firefox.

#### 3. Release Pipeline & Packaging Hardening
- **WiX CDN Decoupling & Target Pinning**: Pinned Tauri bundle targets strictly to `["nsis"]` in `tauri.conf.json` and `scripts/build-release.ps1`, eliminating external WiX 3 toolset download dependencies and preventing CI 504 Gateway Timeout failures.
- **Deterministic CI Execution**: Added `@tauri-apps/cli` to `apps/desktop/package.json` devDependencies and passed `--ci` and `--bundles nsis` to build scripts.
- **Architectural Invariants Verification**: Automated 6/6 pre-flight invariant checks in `scripts/verify-invariants.ps1` covering non-destructive NSIS hooks, bidirectional binary co-location, NMH path resolution, and `WebView2Loader.dll` integrity.
- **Workspace-wide Formatting & Quality Gates**: Applied `cargo fmt` and resolved Clippy lints across all modular packages.

## [v0.0.8] - 2026-11-06

### 🚀 Adaptive Engine V2.0, Media Extraction, Security Hardening & Optimization Overhaul

#### 1. Adaptive Engine V2.0 & High-Throughput Concurrency
- **Win32 `seek_write` Concurrency**: Upgraded `FileWriter` to use non-blocking reader locks on Windows, allowing concurrent parallel workers to write into the pre-allocated `.edmpart` file simultaneously without lock serialization.
- **BBR Congestion Probing & PSP Plateau Locking**: Dynamic scaling algorithm with probe-up, plateau detection, and automatic connection lock to prevent network saturation and packet loss.
- **Micro-Chunk Tail Sniping**: Sub-segments under 5 MiB in the final download phase are dynamically sniped by idle workers to eliminate long trailing tail bottlenecks.
- **Explicit File Handle Lifecycle**: Added `FileWriter::close()` to explicitly release OS file handles on cancellation, pause, and failure.

#### 2. Universal Media & YouTube Integration
- **Host yt-dlp Prioritization & Node.js JS Runtime**: Automatically detects host Node.js binary and passes `--js-runtimes node` to yt-dlp to solve YouTube InnerTube JavaScript challenges.
- **Native In-Process Fallback**: Provides seamless fallback to in-process cipher-free Android InnerTube extractor when yt-dlp is unavailable.
- **YouTube Quick Download Pill**: Injected directly next to YouTube's Subscribe button with dropdown quality selection (1080p, 720p, 480p) and audio-only extraction (MP3 / M4A).
- **SPA Navigation & Slow-Mount Resiliency**: Retries and mutation awareness for YouTube's dynamic client-side DOM rendering.

#### 3. Enterprise Security Hardening
- **Constant-Time Token Verification**: Timing-attack resilient WebSocket token verification powered by `subtle::ConstantTimeEq`.
- **SSRF & Metadata Defense**: Blocks IPv4 link-local (`169.254.0.0/16`) and AWS/cloud IPv6 IMDS endpoints (`[fd00:ec2::254]`), in addition to domain and loopback filters.
- **Safe Explorer Argument Quoting**: Hardened folder and file opening with Windows `raw_arg` syntax `/select,"<path>"` protecting against paths with spaces or special characters.

#### 4. Low-Memory & High-Performance Optimization
- **Active Task Eviction**: `state.tasks` map automatically purges completed, cancelled, and failed tasks, releasing telemetry buffers from memory.
- **Throttled SQLite Persistence**: Progress updates to SQLite are batched to 1.5-second intervals or 1 MiB deltas, reducing disk I/O and mutex contention by over 80%.
- **Throttled WebSocket Emission**: Real-time progress updates are throttled to 250ms, eliminating `tokio::spawn` task storms.
- **Extension Memory Management**: Enforces FIFO-capped intercept maps (max 500 entries) and purges detected media cache on tab removal and navigation.

#### 5. Extension Tactile UI & Connection Resilience
- **Dual-Bevel 3D Design System**: Popup and options redesigned to match the desktop application's visual density, custom scrollbars, and tactile buttons.
- **Connection Recovery Lifecycle**: Non-destructive token caching, automatic loopback HTTP handshake, and Native Messaging Host wake fallback.

## [v0.0.7] - 2026-11-02

### 🚀 Store-Ready Extension & One-Click "Add to <browser>" Flow

#### 1. Multiple Extension IDs & Centralized Configuration
- **Single Source of Truth (`crates/edm-ipc/src/extensions.rs`)**: Unified all allowed extension IDs across Chromium Unpacked (`ginpdimhcdbojbhemgjedghkeolbcmgj`), Chrome Web Store (`ebdcbpghekpbgjiclaclbgmdnimeheci`), Edge Add-ons (`knldjmfmopnpolahpmmgbagdohdnhkik`), Brave Secondary (`cphjfljhggknldkppchfobogknclmblg`), and Firefox Gecko (`edm@suvesh.dev`).
- **Unified Validation**: Shared helper methods `allowed_chromium_origins()`, `allowed_firefox_ids()`, `is_allowed_origin()`, and `is_allowed_extension_id()` govern Native Messaging manifests, WebSocket connection origin checks, HTTP `/token` requests, installer hooks (`hooks.nsh`), repair routines, and diagnostics.
- **Diagnostics Extension ID Visibility**: Diagnostics reports and UI now explicitly display `connected_extension_id` across both desktop Settings and the extension popup. Unit tests verify every allowed ID is accepted and unauthorized IDs/origins are rejected.

#### 2. Store-Ready Package & Documentation
- **Permissions Audit**: Pruned unnecessary permissions (verified `tabs` is omitted; `activeTab` provides scoped access); wrote [`docs/store/PERMISSIONS.md`](docs/store/PERMISSIONS.md) with strict one-line justifications for every retained permission.
- **Zero-Telemetry Privacy Policy**: Authored [`docs/store/PRIVACY.md`](docs/store/PRIVACY.md) ready for GitHub Pages hosting, explicitly documenting zero telemetry, local loopback data boundaries, and cookie handling invariants.
- **Store Listings & Submissions**: Documented official store listing copy, required notices, and screenshot checklists in [`docs/store/LISTING.md`](docs/store/LISTING.md), plus step-by-step submission checklists in [`docs/store/SUBMISSION.md`](docs/store/SUBMISSION.md).
- **Architectural Policy Compliance**: Documented the strict prohibition against registry external extension keys (`HKCU/HKLM\...\Extensions`) in [`docs/store/NO_REGISTRY_POLICY.md`](docs/store/NO_REGISTRY_POLICY.md).
- **CI / Build Packaging**: `extension/build.js` automatically produces `extension/dist/edm-extension-chromium.zip` for Chrome/Edge stores and `extension/dist/edm-firefox.xpi` for Firefox AMO signing via `web-ext sign`.

#### 3. One-Click Flow in Setup Wizard & Settings
- **Configurable Per-Browser Install URLs**: Chrome Web Store URL (Chrome/Brave), Edge Add-ons URL (Edge), and Firefox XPI URL (Firefox) configured with sensible defaults and settings overrides.
- **Direct Browser Exe Launch**: Primary action reads `"Add to <browser>"` and opens the official store URL directly in that browser's own executable (`open_browser_url`), prompting native browser confirmation.
- **Guided Fallback**: If store URLs are unconfigured, smoothly falls back to the guided unpacked flow with automatic folder copying.
- **Live Handshake & Prompt Resiliency**: The wizard actively polls and transitions to "Connected ✓" upon receiving the extension's loopback `hello`. Prompt cancellation leaves the button enabled without error. Firefox is presented only when installed and a signed `.xpi` package exists.

### 🛠️ Hotfixes & Refinements (from v0.0.6 testing)

#### 1. Setup Wizard Persistence & Relaunch Prevention
- **Root Cause**: The Skip button in `SetupWizard.tsx` closed the modal without persisting completion to SQLite storage, causing the wizard to reappear on every application launch.
- **Fix**: Persist `setup_completed` (with backwards compatibility for `wizard_completed`) to the backend SQLite database on both Finish AND Skip. Relaunches and updates never re-show the wizard.
- **Settings Re-entry**: Settings page now includes a dedicated "Run setup again" action to re-run onboarding on demand.

#### 2. Setup Page Redesign (Minimal Silver on Pure Black)
- **Aesthetic**: Redesigned setup wizard with a pure black (`#000000`) canvas, silver typography (`#E4E6EA`, `#B8BCC4`, `#7C818A`, `#1B1C20`), and single-column centered layout constrained to 480 px.
- **3D Tactile Accents**: Primary action styled as a raised silver key with inner highlight, layered depth shadow, and pressed states; browser items rendered as subtly raised plates.
- **Streamlined 3-Screen Flow**: Screen 1: EDM wordmark + concise description (<= 12 words) + Continue & Skip; Screen 2: Detected browsers with one-click Connect / Reinstall; Screen 3: Live native host integration test result and Done.
- **Anti-Marketing Polish**: Stripped unverified feature marketing claims and relocated settings/preferences into the app Settings view.

#### 3. Zero-Error Extension WebSocket State Machine
- **Root Cause**: The background service worker executed unconditional WebSocket connection attempts on startup and retried every 3 seconds while EDM was asleep or closed, triggering browser console connection errors.
- **Fix**: Implemented a non-intrusive status check via the Native Messaging Host (`status`). WebSocket connection is initiated only when EDM is confirmed to be running.
- **Sleep Invariant**: When EDM is asleep or closed, the extension stays completely dormant without background retry intervals.
- **Exponential Backoff**: After unexpected disconnections, the helper is queried once; only if EDM is confirmed running does it reconnect using exponential backoff capped at 60 seconds. Normal sleep states are logged via `console.debug`.

#### 4. Relocated Queue Scheduler & Rules into Settings
- **Clean Chassis**: Removed the standalone Queues navbar button and floating window to maintain visual hierarchy.
- **Queues & Schedule in Settings**: Integrated multi-queue concurrency limits, off-peak schedules, bandwidth throttles, and automated power actions directly into Settings &rarr; "Queues & Schedule".
- **Label Sanitization**: Removed all "(Win32)" designations from action and power options.

#### 5. Native YouTube Watch Page Pill Button & Site Controls
- **Owner Row Placement**: Injected a native-styled action pill button directly into the video owner metadata row immediately after the Subscribe button on `youtube.com/watch` pages.
- **Theme Awareness**: Dynamically adapts colors, borders, and hover states to YouTube's native dark and light themes (36 px height, 18 px pill radius, Roboto typography).
- **SPA & Dynamic DOM Support**: Intercepts YouTube SPA transitions via `yt-navigate-finish`, `spfdone`, and debounced MutationObserver. YouTube Shorts are skipped unless explicitly enabled.
- **Floating Button Cleanup**: Removed the bottom-right floating button overlay on YouTube.
- **Granular Controls**: Added settings for Master Toggle, YouTube Toggle, Other Sites Toggle, and a persistent Hidden Sites domain list with "Hide on this site".

#### 6. Media Sniffer & Link Grabber Reliability; Diagnostics Relocation
- **Top-Level Synchronous Sniffer**: Registered synchronous `webRequest.onResponseStarted` in the service worker to capture video/audio streams, HLS playlists (`.m3u8`), and DASH manifests (`.mpd`) into per-tab `chrome.storage.session`.
- **Extractor Video Probing**: When browsing YouTube or supported media hosts, queries the desktop application's native extraction pipeline to enumerate available video/audio streams and quality formats.
- **Scripting Fallback for Link Grabber**: Added `chrome.scripting.executeScript` fallback to reliably extract links, media, and images even on pages loaded prior to extension installation.
- **Popup Navigation Streamlining**: Reduced popup navigation to 4 focused tabs: **Tasks**, **Media**, **Grab Links**, and **Settings**. Relocated Host Diagnostics into Settings &rarr; Diagnostics with one-click navigation from the header connection pill.

---

## [v0.0.6] - 2026-11-01

### 🚀 Major Features & Additions

#### 1. Expired-Link Recovery ("Refresh Download Address")
- **Mid-Stream Expiry Detection**: Automatically transitions tasks to `LinkExpired` upon HTTP 401, 403, or 410 mid-download, preserving existing partial files without corrupting progress.
- **Safe Overlap Verification**: Probes fresh replacement URLs and verifies both file length and a 1 MiB SHA-256 chunk overlap against existing downloaded bytes before resuming.
- **DPAPI Credential Protection**: Persisted session cookies are encrypted with Windows Data Protection API (`CryptProtectData`) and automatically purged upon task completion.
- **Extension & Manual Refresh**: Trigger refresh directly from browser popup notifications or via the dedicated in-app modal.

#### 2. Queue Scheduler & "When Done" Power Actions
- **Multi-Queue Management**: Configure concurrency limits and speed caps independently per queue.
- **Time-Window Scheduling**: Specify active operational hours (`schedule_start`, `schedule_stop`) and active days of the week, with automatic cross-midnight scheduling support.
- **Win32 Post-Task Actions**: Automated "When Done" triggers for Open Folder, Sleep (`SetSuspendState`), Hibernate, or Shutdown (`InitiateSystemShutdownExW`).
- **60-Second Abort Countdown**: All system power transitions trigger a prominent, cancelable 60-second countdown overlay in the desktop UI and browser extension.

#### 3. Floating Video Downloader & Media Panel
- **Closed Shadow DOM Button**: Zero CSS bleed or page script collision using `mode: 'closed'` custom element overlay on detected `<video>` and streaming elements.
- **Stream Format Probing**: Parses HLS master playlists (`.m3u8`) and direct streams to surface resolution, bitrate, audio channels, and estimated file size.
- **Zero-DRM Invariant**: Strict compliance refusing encrypted EME/Widevine/FairPlay streams with clear user-facing notifications.

#### 4. Batch Download Pattern Expander & Link Grabber
- **Pattern Expansion**: Parses numeric ranges with leading zeros (`[001-100]`), alpha ranges (`[a-z]`), and set expansion (`{1080p,720p}`).
- **Safety Limits & Virtualized Preview**: 200-item batch confirmation warning and a hard 1,000-item expansion cap to prevent memory exhaustion.
- **Duplicate Pre-Flight**: Checks existing database tasks by URL, filename, and file size before enqueuing batch items.

#### 5. First-Run Setup Wizard & App-Managed Extension Updates
- **Step-by-Step Onboarding**: First-run wizard detects Chrome, Edge, Brave, and Firefox installations from the Windows Registry.
- **App-Managed Updates**: Shipped extensions are integrity-checked against an installer SHA-256 manifest and auto-reloaded via `chrome.runtime.reload()` when idle.

#### 6. Diagnostics Suite & Universal Wake Reliability
- **Host & App Diagnostic View**: End-to-end verification for Native Messaging registration, host execution, port readiness, and cold-start timings.
- **One-Click Repair**: Re-registers HKCU native messaging manifests with verified absolute exe paths without requiring administrative privileges.
- **Adaptive Cold-Start Timing**: Extended 20s connection budget prevents premature browser download cancellation during cold app boots.

#### 7. Download Safety & History Ledger
- **Masqueraded Double-Extension Detection**: Flags high-risk executable masking patterns (e.g., `.pdf.exe`, `.jpg.scr`).
- **NTFS Mark-of-the-Web**: Attaches `Zone.Identifier=3` ADS metadata to downloaded payloads matching Windows Defender and SmartScreen security standards.
- **Download History View**: Independent SQLite ledger with search, filtering, and RFC 4180 CSV export.

---

### 🧪 Quality & Verification
- All 45 Rust workspace tests passing (`edm-core`, `edm-store`, `edm-ipc`, `edm-desktop-app`, `chaos_integration`).
- Rust Clippy clean with zero warnings (`-D warnings`).
- Desktop UI (`edm-desktop`) and Browser Extension (`edm-extension`) production builds pass with zero errors.

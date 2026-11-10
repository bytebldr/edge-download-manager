# Changelog

All notable changes to Edge Download Manager (EDM) are documented in this file.

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

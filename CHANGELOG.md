# Changelog

All notable changes to **Evo Download Manager (EDM)** are documented in this file.

## [v0.1.0] - 2026-11-25

### 🚀 Architectural Improvements, Security Hardening & Core Parity Release
- **Modular Architecture & Release Pipeline**: Restructured core engine, native bridge, and desktop modules into isolated feature directories with unified `v0.1.0` release packaging.
- **Adaptive 64-Stream Acceleration**: Synced `max_connections_per_host` with `max_connections`, raised `adaptive_wrapper` and Settings UI slider caps to **64 concurrent streams** per file.
- **Crash-Safe Resume & Server Change Guard**: Added `pause_stale_downloading_tasks()` startup recovery, `store.checkpoint()` on quit, `If-Range` fallback, and `ETag` / `Last-Modified` / `size_total` validation before resuming saved segments.
- **System Tray Mini-Flyout (`#tray-flyout`)**: Added compact tray-anchored popover with queued-top / active-bottom stacking, inline Pause/Resume/Cancel, ETA, and **`Open EDM`** button.
- **Selective BitTorrent File-by-File & Power Controls**: Added `list_only` torrent inspection, `only_files` selective downloading, sequential mode, seeding ratio/time limits, subfolder/rename options, auto-tracker refresh, and IPv4/CIDR peer blocklists.
- **Queues, Scheduler, Batch Import & Media Picker**: Added named queues with overnight-capable `ScheduleWindow`, `.txt`/`.csv`/`.html` batch import + browser page link grabber (`collect_page_links`), native MPEG-DASH (`.mpd`) parser + bounded live recorder, and unified `MediaStreamPickerModal`.
- **Protocols, Routing, i18n & Automation**: Added `.meta4`/`.metalink` parser, injection-safe SFTP/FTPS transfer adapter, per-domain proxy routing, per-category concurrency/speed policies, mid-download `EditRequestModal`, `en`/`es`/`hi`/`ar`(RTL)/`zh` localization, `aria2` JSON-RPC 2.0, completion webhooks, and tray idle memory trim.
- **Security, IP & CI Hardening**: Locked down `/token`, `Hello` extension ID, and `:8998/api` (`x-edm-token`, exact loopback `Host`, allowlisted `Origin`); added SHA-256 + `WinVerifyTrust` update checks; added `PRIVACY.md`, `THIRD_PARTY_NOTICES.md`, `extension/tsconfig.json`, `SHA256SUMS.txt` CI generation, and `docs/01`–`06` audit reports.
- **Modular Refactoring & Cleanup**: Split `core/bridge/src/main.rs` into `{config,nmh,http_api,handlers,main}.rs`, split `services/download_manager.rs` into `{mod,config,telemetry}.rs`, extracted `AppToolbar` / `DownloadContextMenu` / `config/constants.ts`, and removed dead/uncompiled `core/handoff/` and generated `extension/*.js` root artifacts.

---

## [v0.0.1 – v0.0.28] - 2026-11-01 to 2026-11-24

Consolidated early-development (`v0.0.x`) milestones:
- **Core Adaptive Engine & Persistence**: Built the async Rust/Tokio adaptive HTTP engine (`probe`, `planner`, `controller`, `scheduler`, `range_set`, `pipeline_writer`, `TokenBucketLimiter`), SQLite WAL store (`edm-store`), and headless CLI (`edm-cli`).
- **Feature Packs & Extractors**: Integrated in-process `librqbit` BitTorrent/Magnet engine with Tier-1 tracker injection, `yt-dlp` + `ffmpeg` dual-stream pipelining, M3U8/HLS capture, and GitHub/HuggingFace mirror racing.
- **Desktop Shell & Lifecycle Suite**: Built the Tauri v2 + React 19 + Tailwind v4 UI (`SettingsPage`, `DetailsInspectorDrawer`, `SpeedChartInteractive`, `RefreshLinkModal`, `AddDownloadModal`) and frameless lifecycle wizards (`SetupWizard`, `InstallWizard`, `RepairWizard`, `UpdateWizard`, `UninstallWizard`) with Windows NSIS forward-only binary sync (`hooks.nsh`, `installer.nsi`).
- **Browser Companion & Native Messaging**: Implemented `edm-nmh` Native Messaging Host + WebSocket bridge and Manifest V3 extension (`background.ts`, `content.ts`, `popup.ts`, `options.ts`, `yt-main.ts`) with LIFO queue parity and Windows Recycle Bin dual-deletion support.

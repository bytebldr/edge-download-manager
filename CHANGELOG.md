# Changelog

All notable changes to **Evo Download Manager (EDM)** are documented in this file.

## [v0.1.2] - 2026-11-27

### ✨ UI/UX Polish, Capture Confirmation, Security Hardening & Modularity Release
- **Ask Before Download (Capture Confirmation Flow)**: Added `askBeforeDownload` toggle (enabled by default) in **Settings → Downloads & Storage**; captured browser downloads (over both HTTP bridge and WebSocket) now focus EDM and open the pre-filled **Add Download** dialog with **Add** (queue-only), **Download**, and **Cancel** actions.
- **Redesigned Quick Tray, Extension Popup, Scheduler & Add Download Modal**: Unified the dark/light aesthetic across `#tray-flyout` (EDM branding, upload/download footer stats, live appearance sync), `extension` popup (grouped pills, crisp controls), **Queue Scheduler** (start/stop time windows, weekday selectors, and on-complete power actions + navbar quick-add button), and **Add New Download Task** modal.
- **Typography & Font Propagation Fix**: Fixed root CSS variable and `applyFontFamily` propagation so selecting an interface font updates the entire app and tray flyout immediately.
- **Security Hardening (Pre-Release Audit)**: Eliminated DOM XSS in `extension/src/content.ts`, locked down `:8998/api` Origin & CORS allowlists in `extension_bridge.rs`, moved PowerShell toast XML to `EDM_TOAST_XML` env var in `notifications.rs`, blocked loopback/private/unspecified SSRF targets in `validate_url_security`, and added SSRF + path-traversal unit tests.
- **Modularization & Cleanup**: Split `SettingsPage.tsx` tabs into `apps/desktop/src/components/settings/`, broke the `settings.ts` ↔ `tauriBridge.ts` circular import, enforced `tsc --noEmit` in `extension` builds, removed unused root JS copies in `extension/`, and stripped all preview demo data.

---

## [v0.1.1] - 2026-11-26

### 🚀 Auto-Adaptive Engine & High-Density UI De-Box Release
- **Auto-Adaptive Concurrency Engine**: Replaced static stream configuration with an autonomous, self-tuning controller (`core/engine/src/adaptive`). Starts lean at 4 streams, monitors network latency, throughput derivatives, and error signals, and dynamically scales up to 64 streams without manual user configuration.
- **HTTP/2 ALPN & Adaptive Multiplexing**: Enabled `http2` transport negotiation in `reqwest`; throttles HTTP/2 connections to 6 streams per host to avoid server stream resets while scaling HTTP/1.1 connections up to 64.
- **Pipelined Disk Writer & 64 MiB Zero-Copy Buffer**: Implemented `PipelinedWriterHandle` with a 64 MiB global byte-budget semaphore, zero-copy `Bytes` chunks, Windows sparse file preallocation (`set_len` fallback), and synchronous coalesced write-behind I/O on background threads.
- **Linearizable Write Window & Speculative Tail Hedging**: Implemented lock-free write window reservation before chunk ingestion to eliminate disk stall aborts. Added speculative tail hedging to duplicate the slowest remaining worker when the file is >= 95% complete.
- **Crash-Safe Segment Resumption & ETag Invariants**: Upgraded SQLite segment storage (`edm-store`) to update byte boundaries on conflict and prune stale indices. Enforced RFC 9110 §13.1.5 strong ETag checks on `If-Range` to guarantee byte integrity on resume.
- **Monotonic Clocks & Resilient Worker Lifecycle**: Replaced wall-clock intervals with monotonic `Instant` tracking across workers; added request timeouts, randomized retry backoff, RFC 9110 `Retry-After` parsing on 429/503, and graceful `join_all().await` drain on cancel.
- **Frameless UI De-Box & Unified Surface**: Softened dark chassis from `#000000` to `#09090B`, unified header (`h-14`) and footer (`h-8`) outside route transitions, and removed harsh box-like borders across sidebar and action rails.
- **Complete Light Mode Re-engineering**: Eliminated all muddy gray `rgba(0,0,0,...)` gradients and hard black borders. Replaced with clean, crisp, theme-adaptive CSS tokens with WCAG AA compliance across all text labels and badges.
- **Discrete Settings Experience**: Overhauled `SettingsPage` from continuous scrollspy to a focused single-section view with compact `w-48` navigation rail, hairline dividers, and flush swatch pills.
- **Accessible Focus Rings & Typography**: Restored visible `:focus-visible` outline rings across all interactive controls and decoupled `.tabular-nums` from monospace fonts for proportional UI typography.

---

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

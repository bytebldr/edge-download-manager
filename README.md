<p align="center">
  <img src="./assets/logo.png" alt="EDM Download Manager Logo" width="120" height="120" />
</p>

<h1 align="center">Evo Download Manager</h1>

<p align="center">
  <strong>The modern, ultra-fast, open-source download manager and download accelerator for Windows (IDM & FDM Alternative). Built with Rust and Tauri v2.</strong>
</p>

<p align="center">
  <a href="https://github.com/bytebldr/evo-download-manager/releases/latest"><img src="https://img.shields.io/github/v/release/bytebldr/evo-download-manager?color=00f2fe&style=flat-square" alt="Latest Release" /></a>
  <a href="https://github.com/bytebldr/evo-download-manager/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-GPL--3.0-blue?style=flat-square" alt="License" /></a>
  <img src="https://img.shields.io/badge/download--manager-accelerated-00f2fe?style=flat-square" alt="Download Manager" />
  <img src="https://img.shields.io/badge/IDM--alternative-open--source-10b981?style=flat-square" alt="IDM Alternative" />
  <img src="https://img.shields.io/badge/bittorrent-native%20swarm-8b5cf6?style=flat-square" alt="BitTorrent Client" />
  <img src="https://img.shields.io/badge/platform-Windows%2010%20%2F%2011-0078d4?style=flat-square" alt="Platform" />
</p>

---

> [!NOTE]
> **Early Development (`v0.1.3`)**: Evo Download Manager (EDM) `v0.1.3` delivers a self-tuning Auto-Adaptive Concurrency Engine (up to 64 streams), reliable single-concurrency queuing across HTTP/Torrent/YouTube, standalone capture popup, bottom-right Quick Tray, low-RAM background lifecycle, and hardened local IPC security. Feedback and bug reports are welcome via GitHub Issues!

**Evo Download Manager (EDM)** is a next-generation **open-source download manager, torrent client, and download accelerator** for Windows — built as a fast, ad-free alternative to legacy tools like Internet Download Manager (IDM), Free Download Manager (FDM), and JDownloader.

Engineered with a memory-safe **Rust** core and **Tauri v2**, EDM accelerates file transfers with dynamic parallel chunking, in-process BitTorrent & magnet handling, 4K/8K media extraction, and seamless browser extension capture — while suspending idle UI processes so background memory stays near zero.

---

## ✨ Core Capabilities

### 🚀 Adaptive Multi-Connection Download Accelerator
- **Up to 64 Adaptive Parallel Streams**: Dynamically scales from `1` up to `64` concurrent byte-range streams per file when the server supports it, saturating high-speed connections.
- **Dynamic Work-Stealing Tail Sniper**: Automatically detects slow or stalled server connections, splits remaining byte gaps, and reassigns them to faster workers.
- **Crash-Safe Resume & Server Change Guard**: Pause, close your laptop, or lose power — EDM recovers interrupted tasks on startup and checks `ETag`, `Last-Modified`, `If-Range`, and `Content-Length` so a changed file on the server never corrupts your download.
- **Token-Bucket Bandwidth Limiter**: Enforce global or per-category bandwidth limits to download large files without lagging games or video calls.

### 🧲 In-Process BitTorrent & Selective File-by-File Swarm Engine
- **Selective File-by-File Download**: Inspect `.torrent` files and `magnet:` links before starting, pick individual files with size summaries, `Select all` / `Select none`, and create/rename subfolders.
- **Torrent Power Controls**: Sequential download mode, configurable seeding ratio and time limits, automatic Tier-1 tracker refresh, and IPv4/CIDR peer blocklists.
- **Zero Dead-End Downloads**: Opening a `.torrent` file or clicking a `magnet:` link automatically launches the in-process `librqbit` swarm engine.

### 🎥 4K/8K Media, HLS & MPEG-DASH Downloader
- **Media Stream Picker & Live Recording**: Probe `.mpd` (MPEG-DASH), `.m3u8` (HLS), and `yt-dlp` sources to pick video quality, audio tracks, subtitles, or playlist entries — including bounded live stream recording.
- **Dual-Stream Parallel Pipelining**: Simultaneously downloads high-resolution video and audio streams across parallel workers, then stitches them losslessly via FFmpeg (`-c copy`).
- **M3U8 / HLS Stream Capture**: Concurrently fetches transport stream segments with in-process AES-128 decryption.

### 🗂️ Queues, Scheduler, Batch Import & Tray Mini-Flyout
- **System Tray Mini-Flyout**: Click the tray icon for an instant compact card showing queued tasks on top, active downloads at the bottom, inline Pause/Resume/Cancel, ETA, and one-click **`Open EDM`**.
- **Named Queues & Time-Based Scheduler**: Create custom queues with per-queue concurrency caps and overnight-capable `HH:MM` schedule windows.
- **Batch Import & Page Link Grabber**: Import URL lists from `.txt`, `.csv`, or `.html` files, expand numeric ranges (`[1-10]`), or grab all downloadable links from the active browser tab.

### ⚡ Protocols, Routing, Localization & Automation
- **Broad Protocol Support**: HTTP/HTTPS, BitTorrent/Magnet, HLS (`.m3u8`), MPEG-DASH (`.mpd`), Metalink (`.meta4` / `.metalink`), FTP, and SFTP/FTPS.
- **Per-Domain Proxy Routing & Request Overrides**: Route specific domains through custom HTTP/SOCKS5 proxies, enforce per-category speed/concurrency policies, and edit URL, headers, or proxy mid-download.
- **Multi-Language UI (i18n + RTL)**: Built-in localization for English, Español, हिन्दी, العربية (automatic RTL), and 中文.
- **Automation & Idle Memory Trim**: `aria2`-compatible JSON-RPC 2.0 endpoint (token-authenticated), completion webhooks, and Windows tray idle working-set trimming.

### 🧩 Seamless Browser Integration
- **Automatic Download Capture**: Intercepts single-click and right-click downloads across Chrome, Edge, Brave, Opera, and Firefox.
- **Zero-Configuration Connect**: Built-in Native Messaging Host connects to the browser extension automatically.
- **Automatic Wake-on-Download**: EDM stays asleep in the background using near-zero RAM. When a download starts in your browser, EDM wakes up instantly.
- **One-Key Browser Bypass**: Hold `Shift` or `Alt` while clicking any link to download directly through your browser.

### 🛡️ Privacy First, Ad-Free & Open Source
- **100% Free & Open Source**: No advertisements, no premium subscriptions, no tracking, and no bundled bloatware.
- **Zero Cloud Telemetry**: All download tasks, history, and credentials stay strictly on your local machine.

---

## ⚖️ How EDM Compares

| Feature | EDM (Evo Download Manager) | Internet Download Manager (IDM) | Free Download Manager (FDM) | Default Browser Downloader |
| :--- | :---: | :---: | :---: | :---: |
| **License & Price** | **100% Free & Open Source (GPL-3.0)** | Paid (\$24.95 / year) | Free (Closed / Ad-Supported) | Free |
| **User Interface** | **Modern Dark & Light Theme (6 Palettes)** | 90s-era Windows dialogs | Modern | Basic download shelf |
| **Parallel Acceleration** | **Up to 64 Adaptive Streams** | Up to 32 Connections | Multi-segmented | Single connection |
| **BitTorrent & Magnet** | **Built-in In-Process Swarm Engine** | ❌ No Torrent Support | Supported | ❌ No |
| **YouTube & Video Downloader** | **Dual-Stream Parallel Pipeline** | Basic video sniffer | Basic video grabber | ❌ No |
| **Speed Telemetry** | **Real-time 250ms Byte Telemetry** | Averaged / Estimated | Basic graph | Simple progress bar |
| **Memory Efficiency** | **Rust + Tauri v2 (Near-zero idle)** | Low | High memory footprint | High browser memory |
| **Privacy & Security** | **Zero Telemetry, 100% Local** | Closed Source | Closed Source | Tracks with browser account |

---

## 📥 Download & Install

Get the latest production release from the official [**GitHub Releases Page**](https://github.com/bytebldr/evo-download-manager/releases/latest):

| Package | Type | Link |
| :--- | :--- | :--- |
| **EDM Windows Installer** | Standalone Setup (`.exe`) | [**`EDM_0.1.3_x64-setup.exe`**](https://github.com/bytebldr/evo-download-manager/releases/download/v0.1.3/EDM_0.1.3_x64-setup.exe) |
| **Chromium Extension** | Chrome, Edge, Brave, Opera (`.zip`) | [**`edm-extension-chromium.zip`**](https://github.com/bytebldr/evo-download-manager/releases/download/v0.1.3/edm-extension-chromium.zip) |
| **Firefox Add-on** | Mozilla Firefox (`.xpi`) | [**`edm-firefox.xpi`**](https://github.com/bytebldr/evo-download-manager/releases/download/v0.1.3/edm-firefox.xpi) |
| **Extension Source** | Unpacked Bundle Archive (`.tar.gz`) | [**`edm-extension.tar.gz`**](https://github.com/bytebldr/evo-download-manager/releases/download/v0.1.3/edm-extension.tar.gz) |
| **Release Checksums** | SHA-256 Manifest (`.txt`) | [**`SHA256SUMS.txt`**](https://github.com/bytebldr/evo-download-manager/releases/download/v0.1.3/SHA256SUMS.txt) |

### Installing the Browser Companion:
1. Download `edm-extension-chromium.zip` (for Chrome, Edge, Brave, Opera) or `edm-firefox.xpi` (for Firefox).
2. For Chromium browsers:
   - Extract the `.zip` archive.
   - Go to `chrome://extensions` or `edge://extensions`.
   - Enable **Developer mode** (top-right switch).
   - Click **Load unpacked** and select the extracted folder (or `extension/dist/chromium` if building from source).
3. The extension connects automatically to EDM and is ready to capture downloads!

---

## ❓ Frequently Asked Questions (FAQ)

### Is EDM a free alternative to Internet Download Manager (IDM)?
Yes! EDM is 100% free and open source under the GPL-3.0 license. Unlike IDM, EDM does not require a paid license, has zero trial limits, contains zero advertisements, and includes modern features such as native BitTorrent support, 4K video extraction, and an intuitive dark theme interface.

### How does EDM download files faster than browsers?
Standard web browsers download files through a single HTTP connection, which is often throttled by remote servers or high-latency network routes. EDM divides files into multiple byte-range segments and downloads them simultaneously over up to 64 adaptive parallel connections, fully utilizing your available bandwidth.

### Can EDM download torrent files and magnet links?
Yes. EDM includes an in-process BitTorrent client powered by `librqbit`. It resolves `magnet:` links, downloads `.torrent` files, contacts Tier-1 public trackers, and supports sequential streaming so you can watch video files while they download.

### Does EDM collect any user data or browsing telemetry?
No. EDM operates 100% locally on your computer. It never communicates with any third-party tracking services or analytics servers. Your downloads, links, and browsing history stay completely private.

---

## 🛠️ Building from Source

Prerequisites: **Rust** (stable toolchain), **Node.js** (20+ LTS), and **Visual Studio C++ Build Tools** (Windows SDK).

```bash
# 1. Clone the repository
git clone https://github.com/bytebldr/evo-download-manager.git
cd evo-download-manager

# 2. Build the browser companion extension
cd extension && npm install && npm run build && cd ..

# 3. Build the desktop UI & run tests
cd apps/desktop && npm install && npm run build && cd ../..
cargo test --workspace

# 4. Run the unified release installer build (Windows)
.\scripts\build-release.ps1
```

---

## 🔒 Security & Privacy

- [**Architecture Overview**](docs/02-new-architecture.md) — Modular directory map across `core/engine`, `core/bridge`, `apps/desktop`, and `extension`.
- [**Contributing Guide**](docs/CONTRIBUTING.md) — Local setup, coding standards, and pull request workflow.
- [**Privacy Policy**](PRIVACY.md) — Details on local-only network activity, zero telemetry, and browser extension permissions (Chrome Web Store Limited Use compliant).
- [**Security Policy**](docs/SECURITY.md) — Vulnerability reporting and local IPC/integrity guarantees.
- [**Third-Party Notices**](THIRD_PARTY_NOTICES.md) — Microsoft Edge WebView2 Redistributable, Tauri NSIS attribution, and open-source dependency licenses.

---

## 📄 License

Evo Download Manager is released under the [**GNU General Public License v3.0 (GPL-3.0)**](LICENSE).




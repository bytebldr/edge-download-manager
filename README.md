<p align="center">
  <img src="./assets/logo.png" alt="EDM Download Manager Logo" width="120" height="120" />
</p>

<h1 align="center">EDM — Edge Download Manager</h1>

<p align="center">
  <strong>The modern, ultra-fast, open-source download manager and download accelerator for Windows (IDM & FDM Alternative). Built with Rust and Tauri v2.</strong>
</p>

<p align="center">
  <a href="https://github.com/bytebldr/edge-download-manager/releases/latest"><img src="https://img.shields.io/github/v/release/bytebldr/edge-download-manager?color=00f2fe&style=flat-square" alt="Latest Release" /></a>
  <a href="https://github.com/bytebldr/edge-download-manager/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-GPL--3.0-blue?style=flat-square" alt="License" /></a>
  <img src="https://img.shields.io/badge/download--manager-accelerated-00f2fe?style=flat-square" alt="Download Manager" />
  <img src="https://img.shields.io/badge/IDM--alternative-open--source-10b981?style=flat-square" alt="IDM Alternative" />
  <img src="https://img.shields.io/badge/bittorrent-native%20swarm-8b5cf6?style=flat-square" alt="BitTorrent Client" />
  <img src="https://img.shields.io/badge/platform-Windows%2010%20%2F%2011-0078d4?style=flat-square" alt="Platform" />
</p>

---

**Edge Download Manager (EDM)** is a next-generation **open-source download manager, torrent client, and download accelerator**. Designed as a high-performance, ad-free alternative to legacy download managers like Internet Download Manager (IDM), Free Download Manager (FDM), and JDownloader. 

Engineered with a memory-safe **Rust** core and **Tauri v2**, EDM accelerates file transfers up to 10x using dynamic parallel chunking, automatic BitTorrent & magnet handling, 4K/8K media extraction, and seamless browser extension capture.

---

## 📸 Application Showcase

### Modern Desktop Dashboard
Organize, manage, and monitor all your downloads in a distraction-free, high-density interface with instant filtering by videos, music, archives, documents, and programs.

<p align="center">
  <img src="https://raw.githubusercontent.com/bytebldr/edge-download-manager/main/assets/screenshots/edm-desktop-dashboard-v0.0.5.png" alt="EDM Download Manager Dashboard" width="900" />
</p>

### Live Speed Telemetry & Multi-Connection Inspector
Inspect genuine transfer speeds, real-time throughput graphs, and per-connection worker progress with zero synthetic numbers or fake peaks.

<p align="center">
  <img src="https://raw.githubusercontent.com/bytebldr/edge-download-manager/main/assets/screenshots/edm-telemetry-inspector-v0.0.5.png" alt="EDM Download Manager Live Speed Telemetry" width="720" />
</p>

### Seamless Browser Companion Extension
Catch downloads instantly from your favorite browser. Check live progress, pause, resume, or open files directly from the browser popup without switching windows.

<p align="center">
  <img src="https://raw.githubusercontent.com/bytebldr/edge-download-manager/main/assets/screenshots/edm-browser-extension-v0.0.5.png" alt="EDM Download Manager Browser Extension" width="440" />
</p>

---

## ✨ Why EDM is the Best Download Manager for Windows

### 🚀 Adaptive Multi-Connection Download Accelerator
- **Up to 32 Parallel Streams**: Splits files dynamically into concurrent byte-range streams to saturate your internet connection and download at the maximum speed your network can deliver.
- **Dynamic Work-Stealing Tail Sniper**: Automatically detects slow or stalled server connections, splits remaining byte gaps, and reassigns them to faster workers.
- **Token-Bucket Bandwidth Limiter**: Enforce global bandwidth limits to download large files without lagging games, video calls, or household Wi-Fi.

### 🧲 In-Process BitTorrent & Magnet Download Manager
- **Zero Dead-End Downloads**: Opening a `.torrent` file or clicking a `magnet:` link automatically launches the in-process swarm engine to download your content—never leaving you stranded with an inert metadata file.
- **Curated Tier-1 Public Tracker Injection**: Automatically injects high-availability public trackers into sparse magnet links for instant DHT peer discovery in under 3 seconds.
- **Sequential Play-While-Downloading (VOD)**: Prioritizes video pieces sequentially so you can stream and preview `.mp4` and `.mkv` media files before the download finishes.

### 🎥 4K/8K Media & Video Stream Downloader
- **YouTube Quick-Download Pill**: Integrates a clean, compact download button directly into YouTube watch pages and shorts next to the Subscribe button.
- **Dual-Stream Parallel Pipelining**: Simultaneously downloads high-resolution video and audio streams across 16 parallel workers, then stitches them losslessly via in-process FFmpeg (`-c copy`) in under 1 second.
- **M3U8 / HLS Stream Capture**: Concurrently fetches transport stream segments with in-process AES-128 decryption.

### ⚡ Intelligent Upfront & Adaptive Wrapper Subsystem
- **Speculative Sub-600ms Mirror Racing**: Automatically pings origin and CDN mirrors in parallel for GitHub releases and Hugging Face models, routing workers to the fastest responsive endpoint.
- **Credential & Security Isolation**: Strips sensitive authorization headers and session cookies before contacting third-party mirrors.
- **Cryptographic Hash Verification**: Automatically validates downloaded files against SHA-256, Blake3, and Git-LFS checksums.

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

| Feature | EDM (Edge Download Manager) | Internet Download Manager (IDM) | Free Download Manager (FDM) | Default Browser Downloader |
| :--- | :---: | :---: | :---: | :---: |
| **License & Price** | **100% Free & Open Source (GPL-3.0)** | Paid (\$24.95 / year) | Free (Closed / Ad-Supported) | Free |
| **User Interface** | **Modern Dark Theme & 3D Glass UI** | 90s-era Windows dialogs | Modern | Basic download shelf |
| **Parallel Acceleration** | **Up to 32 Adaptive Streams** | Up to 32 Connections | Multi-segmented | Single connection |
| **BitTorrent & Magnet** | **Built-in In-Process Swarm Engine** | ❌ No Torrent Support | Supported | ❌ No |
| **YouTube & Video Downloader** | **Dual-Stream Parallel Pipeline** | Basic video sniffer | Basic video grabber | ❌ No |
| **Speed Telemetry** | **Real-time 250ms Byte Telemetry** | Averaged / Estimated | Basic graph | Simple progress bar |
| **Memory Efficiency** | **Rust + Tauri v2 (Near-zero idle)** | Low | High memory footprint | High browser memory |
| **Privacy & Security** | **Zero Telemetry, 100% Local** | Closed Source | Closed Source | Tracks with browser account |

---

## 📥 Download & Install

Get the latest production release from the official [**GitHub Releases Page**](https://github.com/bytebldr/edge-download-manager/releases/latest):

| Package | Type | Link |
| :--- | :--- | :--- |
| **EDM Windows Installer** | Standalone Setup (`.exe`) | [**`EDM_0.0.11_x64-setup.exe`**](https://github.com/bytebldr/edge-download-manager/releases/download/v0.0.11/EDM_0.0.11_x64-setup.exe) |
| **Chromium Extension** | Chrome, Edge, Brave, Opera (`.zip`) | [**`edm-extension-chromium.zip`**](https://github.com/bytebldr/edge-download-manager/releases/download/v0.0.11/edm-extension-chromium.zip) |
| **Firefox Add-on** | Mozilla Firefox (`.xpi`) | [**`edm-firefox.xpi`**](https://github.com/bytebldr/edge-download-manager/releases/download/v0.0.11/edm-firefox.xpi) |
| **Extension Source** | Unpacked Bundle Archive (`.tar.gz`) | [**`edm-extension.tar.gz`**](https://github.com/bytebldr/edge-download-manager/releases/download/v0.0.11/edm-extension.tar.gz) |

### Installing the Browser Companion:
1. Download `edm-extension-chromium.zip` (for Chrome, Edge, Brave, Opera) or `edm-firefox.xpi` (for Firefox).
2. For Chromium browsers:
   - Extract the `.zip` archive.
   - Go to `chrome://extensions` or `edge://extensions`.
   - Enable **Developer mode** (top-right switch).
   - Click **Load unpacked** and select the extracted folder.
3. The extension connects automatically to EDM and is ready to capture downloads!

---

## ❓ Frequently Asked Questions (FAQ)

### Is EDM a free alternative to Internet Download Manager (IDM)?
Yes! EDM is 100% free and open source under the GPL-3.0 license. Unlike IDM, EDM does not require a paid license, has zero trial limits, contains zero advertisements, and includes modern features such as native BitTorrent support, 4K video extraction, and an intuitive dark theme interface.

### How does EDM download files faster than browsers?
Standard web browsers download files through a single HTTP connection, which is often throttled by remote servers or high-latency network routes. EDM divides files into multiple byte-range segments and downloads them simultaneously over up to 32 parallel connections, fully utilizing your available bandwidth.

### Can EDM download torrent files and magnet links?
Yes. EDM includes an in-process BitTorrent client powered by `librqbit`. It resolves `magnet:` links, downloads `.torrent` files, contacts Tier-1 public trackers, and supports sequential streaming so you can watch video files while they download.

### Does EDM collect any user data or browsing telemetry?
No. EDM operates 100% locally on your computer. It never communicates with any third-party tracking services or analytics servers. Your downloads, links, and browsing history stay completely private.

---

## 📄 License & Third-Party Credits

EDM is released under the **GNU General Public License v3.0 (GPL-3.0)**.  
For open-source attributions and third-party engine credits, see [**THIRD_PARTY_CREDITS.md**](THIRD_PARTY_CREDITS.md).

---

<sub>**Keywords**: download manager, downloadmanager, internet download manager, idm alternative, fdm alternative, download accelerator, windows download manager, open source download manager, bittorrent client, torrent download manager, magnet link downloader, video downloader, youtube downloader, 4k video downloader, hls downloader, m3u8 downloader, multithreaded download manager, rust download manager, tauri v2.</sub>

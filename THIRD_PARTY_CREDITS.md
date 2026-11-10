# Third-Party Credits & Open-Source Attributions

Edge Download Manager (EDM) is built upon and integrates exceptional open-source software, protocols, and developer libraries. This document acknowledges and gives credit to the third-party upstream projects, engines, and dependencies that power EDM's feature packs and core architecture.

---

## 1. Feature Pack 3rd-Party Engines & Sources

### 🧲 BitTorrent Swarm Pack
* **librqbit**
  * **Role in EDM**: Powers EDM's in-process BitTorrent client, magnet link resolution, DHT (Kademlia), uTP/TCP peer-wire communication, PEX, and sequential piece downloading without external daemon dependencies.
  * **Upstream**: [https://github.com/ikatson/rqbit](https://github.com/ikatson/rqbit)
  * **Author / Maintainers**: Igor Gnatenko and librqbit contributors
  * **License**: Apache-2.0 / MIT

---

### 🎥 Media Site Extractor Pack
* **yt-dlp**
  * **Role in EDM**: Serves as the primary reference and optional external execution engine for extracting high-resolution video/audio streams, multi-format listing, and resolving media signatures across complex platforms (YouTube, Vimeo, etc.).
  * **Upstream**: [https://github.com/yt-dlp/yt-dlp](https://github.com/yt-dlp/yt-dlp)
  * **Authors / Maintainers**: yt-dlp contributors
  * **License**: Unlicense / Public Domain

---

### 🎞️ Native Media Remux Pack
* **FFmpeg**
  * **Role in EDM**: Used as the optional companion toolchain for lossless stream-copy transmuxing (`-c copy`), audio/video container stitching, and demuxing when complex container remuxing is required.
  * **Upstream**: [https://ffmpeg.org](https://ffmpeg.org)
  * **Authors / Maintainers**: FFmpeg team and contributors
  * **License**: LGPL v2.1+ / GPL v2+

---

### 📺 M3U8 / HLS Stream Pack
* **RustCrypto (`aes`, `cbc`)**
  * **Role in EDM**: Provides in-process, pure Rust AES-128-CBC cipher decryption for encrypted HTTP Live Streaming (HLS) transport stream (`.ts`) segments without requiring external command-line utilities.
  * **Upstream**: [https://github.com/RustCrypto](https://github.com/RustCrypto)
  * **Authors / Maintainers**: RustCrypto project developers
  * **License**: MIT / Apache-2.0

---

### ⚡ GitHub Releases & Accelerators Pack
* **ghproxy**
  * **Role in EDM**: Provides global mirror acceleration endpoints for high-speed routing of release assets and binary archives.
  * **Upstream**: [https://ghproxy.net](https://ghproxy.net)
  * **License**: Open-Source / Public Mirror Service

---

### 🤗 Hugging Face Models & Dataset Pack
* **HF-Mirror**
  * **Role in EDM**: Accelerated CDN endpoint for high-throughput downloads of Safetensors, GGUF shards, and model weights.
  * **Upstream**: [https://hf-mirror.com](https://hf-mirror.com)
  * **License**: Public Mirror Service

---

## 2. Core Systems & Runtime Libraries

| Dependency | Purpose | Upstream Link | License |
|---|---|---|---|
| **Tauri v2** | Desktop application framework and native Windows webview bridge | [tauri.app](https://tauri.app) | MIT / Apache-2.0 |
| **Tokio** | Asynchronous runtime, task scheduling, timers, and process supervisor | [tokio.rs](https://tokio.rs) | MIT |
| **Reqwest** | High-performance asynchronous HTTP/1.1 and HTTP/2 client | [github.com/seanmonstar/reqwest](https://github.com/seanmonstar/reqwest) | MIT / Apache-2.0 |
| **Rustls** | Modern, memory-safe TLS 1.3 implementation | [github.com/rustls/rustls](https://github.com/rustls/rustls) | Apache-2.0 / MIT / ISC |
| **Rusqlite** | Ergonomic SQLite wrapper for local download persistence and metadata | [github.com/rusqlite/rusqlite](https://github.com/rusqlite/rusqlite) | MIT |
| **SQLite** | Embedded relational SQL database engine | [sqlite.org](https://sqlite.org) | Public Domain |
| **Axum & Tower** | In-process mock testing server and HTTP middleware utilities | [github.com/tokio-rs/axum](https://github.com/tokio-rs/axum) | MIT |
| **windows-registry** | Native Windows Registry manipulation for protocol and file type handlers | [github.com/microsoft/windows-rs](https://github.com/microsoft/windows-rs) | MIT / Apache-2.0 |

---

## 3. Desktop Frontend & UI Libraries

| Dependency | Purpose | Upstream Link | License |
|---|---|---|---|
| **React 19** | User interface component architecture | [react.dev](https://react.dev) | MIT |
| **Tailwind CSS v4** | Modern utility-first styling and design system | [tailwindcss.com](https://tailwindcss.com) | MIT |
| **Motion** | Fluid layout morphing, spring physics, and micro-interactions | [motion.dev](https://motion.dev) | MIT |
| **Lucide Icons** | Clean, accessible SVG iconography | [lucide.dev](https://lucide.dev) | ISC |
| **Recharts** | Interactive real-time network throughput and speed charts | [recharts.org](https://recharts.org) | MIT |
| **Zustand** | Minimalist reactive global state management | [github.com/pmndrs/zustand](https://github.com/pmndrs/zustand) | MIT |

---

## 4. Trademarks & Compliance

* All product names, logos, brands, and registered trademarks mentioned are property of their respective owners.
* Reference to third-party tools (such as YouTube, Vimeo, GitHub, Hugging Face, FFmpeg, yt-dlp) is solely for descriptive and interoperability purposes and does not imply endorsement or official affiliation.

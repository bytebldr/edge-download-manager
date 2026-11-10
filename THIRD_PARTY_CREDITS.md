# Third-Party Feature Pack Credits & Attributions

This document acknowledges and gives credit to the third-party upstream projects, engines, and services that power Edge Download Manager's feature packs.

---

## Feature Pack 3rd-Party Engines & Sources

### 🧲 BitTorrent Swarm Pack
* **librqbit**
  * **Pack Capability**: In-process BitTorrent client engine providing magnet link resolution, DHT (Kademlia), uTP/TCP peer-wire communication, Peer Exchange (PEX), and piece-level downloading without external daemon dependencies.
  * **Upstream**: [https://github.com/ikatson/rqbit](https://github.com/ikatson/rqbit)
  * **Author / Maintainers**: Igor Gnatenko and librqbit contributors
  * **License**: Apache-2.0 / MIT

---

### 🎥 Media Site Extractor Pack
* **yt-dlp**
  * **Pack Capability**: Video and audio stream metadata extractor supporting multi-format listing, resolution selection, and media signature decryption across web platforms (YouTube, Vimeo, etc.).
  * **Upstream**: [https://github.com/yt-dlp/yt-dlp](https://github.com/yt-dlp/yt-dlp)
  * **Authors / Maintainers**: yt-dlp contributors
  * **License**: Unlicense / Public Domain

---

### 🎞️ Native Media Remux Pack
* **FFmpeg**
  * **Pack Capability**: Multimedia framework providing stream demuxing, lossless stream-copy transmuxing (`-c copy`), and audio/video container stitching.
  * **Upstream**: [https://ffmpeg.org](https://ffmpeg.org)
  * **Authors / Maintainers**: FFmpeg team and contributors
  * **License**: LGPL v2.1+ / GPL v2+

---

### 📺 M3U8 / HLS Stream Pack
* **RustCrypto (`aes`, `cbc`)**
  * **Pack Capability**: In-process AES-128-CBC cipher implementation providing synchronous and streaming cryptographic decryption for encrypted HTTP Live Streaming (HLS) transport stream (`.ts`) segments.
  * **Upstream**: [https://github.com/RustCrypto](https://github.com/RustCrypto)
  * **Authors / Maintainers**: RustCrypto project developers
  * **License**: MIT / Apache-2.0

---

### ⚡ GitHub Releases & Accelerators Pack
* **ghproxy**
  * **Pack Capability**: Distributed mirror proxy accelerating download bandwidth and routing for release assets and binary archives.
  * **Upstream**: [https://ghproxy.net](https://ghproxy.net)
  * **License**: Open-Source / Public Mirror Service

---

### 🤗 Hugging Face Models & Dataset Pack
* **HF-Mirror**
  * **Pack Capability**: Dedicated mirror endpoint providing accelerated transfer speeds for Safetensors, GGUF shards, and model weights.
  * **Upstream**: [https://hf-mirror.com](https://hf-mirror.com)
  * **License**: Public Mirror Service

---

## Trademarks & Compliance

* All product names, logos, brands, and registered trademarks mentioned are property of their respective owners.
* Reference to third-party tools and services (such as YouTube, Vimeo, GitHub, Hugging Face, FFmpeg, yt-dlp) is solely for descriptive and interoperability purposes and does not imply endorsement or official affiliation.

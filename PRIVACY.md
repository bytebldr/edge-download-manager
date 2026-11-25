# Privacy Policy

**Last updated:** 2026-11-25  
**Project:** Evo Download Manager (EDM) (`https://github.com/bytebldr/evo-download-manager`)

Evo Download Manager (EDM) is a local-first, open-source download manager and browser companion. **EDM collects zero telemetry, zero analytics, and zero personal data.** All download tasks, history, and credentials remain strictly on your local device.

---

## 1. Desktop Application Network Activity

The EDM desktop application (`EDM.exe`) and its local native messaging host (`edm-nmh.exe`) only initiate network connections for explicit, user-expected functionality:

1. **Software Updates**: When checking for or applying updates, EDM queries the official GitHub Releases API (`https://api.github.com/repos/bytebldr/evo-download-manager/releases`) and downloads release installers and SHA-256 checksums from `github.com/bytebldr/evo-download-manager`.
2. **User-Initiated Downloads**: EDM connects directly to the HTTP/HTTPS URLs and CDN/release mirrors you choose to download. Sensitive authorization headers and cookies are stripped before probing third-party mirrors.
3. **BitTorrent & Magnet Links**: When you open a `.torrent` file or `magnet:` link, the in-process BitTorrent engine (`librqbit`) connects to BitTorrent peers, DHT nodes, and configured Tier-1 public trackers to discover peers and transfer pieces.
4. **Zero Telemetry & Zero Cloud Sync**: EDM does not send usage metrics, crash reports, device identifiers, or download history to any developer-operated or third-party analytics server. All state is stored locally in your local SQLite database.

---

## 2. Browser Extension Permissions & Data Handling

The EDM browser extension communicates exclusively with your local EDM desktop application over localhost (`127.0.0.1`) and Chrome/Firefox Native Messaging (`edm-nmh`). No browsing data, URLs, or cookies are ever sent to any external server.

### Why Each Permission Is Requested

| Permission | Purpose |
| :--- | :--- |
| `cookies` | Reads session cookies solely for the target download URL when handing off an authenticated user-initiated download to the local EDM desktop app. |
| `downloads` | Intercepts and manages browser download events so downloads can be routed to EDM for multi-connection acceleration (with `Shift`/`Alt` bypass support). |
| `nativeMessaging` | Communicates locally with `edm-nmh.exe` to wake the desktop app, exchange the local pairing token, and query installation status. |
| `storage` | Stores local extension preferences (e.g., auto-capture toggle, local pairing token, deletion policy) in `chrome.storage` / `browser.storage`. |
| `<all_urls>` | Required to detect downloadable links, media streams, and request headers/cookies on pages you visit for strictly local handoff to EDM. |

### Chrome Web Store Limited Use Compliance

The use of information received from Google Chrome / Chromium APIs adheres to the [**Chrome Web Store User Data Policy**](https://developer.chrome.com/docs/webstore/program-policies/user-data-privacy), including the **Limited Use** requirements:
- Data is used solely to provide and improve the single-purpose local download management functionality requested by the user.
- Data is never transferred to third parties, sold, or used for advertising, profiling, or creditworthiness purposes.
- Human beings do not read user data.

---

## 3. Contact

If you have privacy questions or concerns, please open an issue at:  
`https://github.com/bytebldr/evo-download-manager/issues`

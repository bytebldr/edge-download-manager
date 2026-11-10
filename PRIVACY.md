# Privacy Policy

**Effective Date**: November 2026  
**Project**: Edge Download Manager (EDM)  
**Repository**: [github.com/bytebldr/edge-download-manager](https://github.com/bytebldr/edge-download-manager)

Edge Download Manager is committed to user privacy. We believe a download manager should strictly serve the user without surveilling their browsing habits, download history, or personal data.

---

## 1. Zero Telemetry & Tracking Invariant

- **No Remote Telemetry**: Edge Download Manager contains **zero** remote analytics, behavioral trackers, crash-reporting SDKs, or phone-home telemetry services.
- **No Third-Party Analytics**: We do not embed Google Analytics, Sentry, Mixpanel, or any tracking libraries in either the desktop application or browser extensions.
- **No Adware or PUPs**: EDM is 100% free of advertising, sponsored toolbars, crypto-miners, and bundled third-party installers.

---

## 2. Local Data Storage

All application state is stored strictly on your local machine:
- **Download Database**: Stored in a local, WAL-enabled SQLite database at `%LOCALAPPDATA%\EDM\edm.db`.
- **Application Settings**: Stored locally in JSON format at `%LOCALAPPDATA%\EDM\settings.json`.
- **Browser Extension Settings**: Stored within the browser's local sandbox storage (`chrome.storage.local`).
- **Log Files**: Stored locally in `%LOCALAPPDATA%\EDM\logs\`. Logs are never transmitted over the network and remain solely under the user's control.

---

## 3. Network Communication

Edge Download Manager communicates over the network **only** when performing explicit actions initiated by the user:

1. **User-Requested Downloads**:
   - Outgoing HTTP/HTTPS, FTP, and BitTorrent connections are established exclusively to retrieve the specific files or media URLs the user requests to download.
   - Headers, cookies, and authentication tokens provided for a download are sent only to the host domain requested by the user.
2. **Version Update Checks** *(Optional & Configurable)*:
   - When enabled, EDM checks the public GitHub Releases API (`https://api.github.com/repos/bytebldr/edge-download-manager/releases/latest`) to determine if a newer version is available.
   - No user identity, machine fingerprint, or hardware telemetry is attached to this request.
3. **Local Loopback Communication**:
   - The desktop app and browser extension communicate strictly over local loopback (`127.0.0.1`) via WebSockets and Windows Native Messaging. This traffic never leaves the local machine.

---

## 4. Open Source & Verifiable

Edge Download Manager is 100% open source under the **GNU General Public License v3.0**. Anyone can independently audit our source code and verify our zero-telemetry claims at [github.com/bytebldr/edge-download-manager](https://github.com/bytebldr/edge-download-manager).

---

## 5. Contact

For privacy questions or inquiries regarding our data handling principles, contact `edm@suvesh.dev` or open an issue on GitHub.

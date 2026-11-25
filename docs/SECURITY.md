# Security Policy

## Supported Versions

| Version | Supported |
| :--- | :--- |
| `0.1.x` | ✅ Yes |
| `< 0.1.0` | ❌ No (upgrade to latest release) |

---

## Reporting a Vulnerability

If you discover a security vulnerability in **Evo Download Manager (EDM)** (`https://github.com/bytebldr/evo-download-manager`), please report it privately rather than opening a public issue:

1. Use **GitHub Security Advisories** ("Report a vulnerability" on the repository's **Security** tab).
2. Include a clear description, reproduction steps, affected component (desktop app, native messaging host, or browser extension), and potential impact.

We aim to acknowledge reports within 72 hours and provide a fix or mitigation plan promptly.

---

## Security Architecture Highlights

- **Local-Only IPC**: The desktop WebSocket/HTTP bridge binds to `127.0.0.1` only, enforces exact `Host` and allowed extension `Origin` checks, and uses constant-time token verification (`subtle`).
- **Path & Execution Safety**: File paths are canonicalized and containment-checked; dangerous executable extensions are not auto-executed directly.
- **Update Integrity**: OTA updates verify SHA-256 checksums before launching the installer.

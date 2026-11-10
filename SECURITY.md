# Security Policy

Edge Download Manager (EDM) takes the security and integrity of our software and users' systems seriously. We appreciate responsible disclosure of any security vulnerabilities.

---

## Supported Versions

Only the latest release of Edge Download Manager receives security updates and bug fixes:

| Version | Supported |
| :--- | :--- |
| **>= 0.0.12** | :white_check_mark: Yes |
| < 0.0.12 | :x: No (Please upgrade to the latest release) |

---

## Defensive Security Architecture

Edge Download Manager is engineered with strict defensive security invariants:

1. **Strict Input Validation**:
   - 100% of external URLs, headers, and protocol inputs are sanitized and validated with strict bounds before file allocation or process execution.
2. **Safe IPC & Browser Origin Isolation**:
   - WebSocket (`127.0.0.1:14370`) and Native Messaging connections strictly enforce browser extension ID whitelisting and CSP origin validation. Arbitrary web pages cannot trigger downloads or execute commands.
3. **Restricted File Execution**:
   - Files with high-risk executable extensions (`.exe`, `.bat`, `.cmd`, `.vbs`, `.ps1`, `.msi`) trigger explicit warnings and require direct user interaction before opening.
4. **Zero Telemetry & Local Data Isolation**:
   - Zero analytics, zero usage telemetry, and zero behavioral tracking.
   - All download metadata, credentials, and settings remain strictly stored in local SQLite databases on the user's local machine (`%LOCALAPPDATA%\EDM\edm.db`).

---

## Reporting a Vulnerability

If you discover a security vulnerability or supply chain issue in Edge Download Manager, **please do not disclose it in public issues, discussions, or pull requests**.

### Preferred Reporting Method
- **GitHub Security Advisory**: Submit a private vulnerability report via [GitHub Security Advisories](https://github.com/bytebldr/edge-download-manager/security/advisories/new).
- **Email Contact**: Send an encrypted or direct email to `edm@suvesh.dev` with the subject `[SECURITY] Edge Download Manager Vulnerability Report`.

### What to Include in Your Report
Please provide:
- A detailed description of the vulnerability and its potential impact.
- Step-by-step instructions or Proof of Concept (PoC) to reproduce the issue.
- The affected component(s) (`apps/desktop`, `packages/engine`, `packages/browser-host`, `extension`).
- Any suggested mitigations or patches if available.

### Response Timeline
- **Initial Acknowledgment**: Within **24 to 48 hours**.
- **Assessment & Triage**: Within **3 business days**.
- **Fix & Public Advisory**: Coordinated release published within **14 business days**, crediting the reporter (unless anonymity is requested).

Thank you for helping keep Edge Download Manager and our community secure.

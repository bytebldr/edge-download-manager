# EDM Extension ↔ Desktop Handoff Subsystem (`core/handoff`)

## 1. Overview & Purpose
This directory houses the isolated communication pipeline between the browser extension and the EDM desktop application.

```
┌────────────────────────┐         ┌────────────────────────┐         ┌────────────────────────┐
│   Browser Extension    │ ──────> │  edm-nmh.exe (Bridge)  │ ──────> │    EDM.exe (Desktop)   │
│  (MV3 Service Worker)  │ <────── │ (Native Messaging Host)│ <────── │  (WebSocket on 14370)  │
└────────────────────────┘         └────────────────────────┘         └────────────────────────┘
```

By isolating all communication contracts, bridge helpers, host listeners, and test harnesses in `core/handoff/`:
* **Zero UI Blast Radius**: Changes to desktop UI or download algorithms never impact or destabilize the communication handshake.
* **Deterministic Single Source of Truth**: All port numbers (`14370`), token generation logic, browser registry keys, and protocol schemas live here.
* **Hermetic Testability**: The communication subsystem can be verified 100% in isolation using the enclosed test harness without launching WebView2 or the GUI.

---

## 2. Directory Architecture

```
core/handoff/
├── README.md               # Subsystem architecture contract & maintenance protocol
├── contract/               # Dual-language protocol definitions
│   ├── types.rs            # Canonical Rust message schemas & semver comparisons
│   └── types.ts            # Generated/synchronized TypeScript interfaces for the extension
├── host/                   # In-app WebSocket & HTTP Token Server
│   ├── listener.rs         # Resilient TCP/WS listener with immortal retry loop
│   ├── auth.rs             # Constant-time token verification & extension origin whitelist
│   └── lifecycle.rs        # Auto-wake, active, warm, listener_sleep, and deep sleep manager
├── bridge/                 # Browser Native Messaging Host (edm-nmh)
│   ├── spawn.rs            # Detached process launcher with GUI-safe flags
│   ├── probe.rs            # Zero-hang loopback port 14370 health probe
│   └── manifest.rs         # Chromium & Firefox native messaging manifest installer
└── test-harness/           # Standalone verification pipeline
    ├── mock_extension.mjs  # Headless test runner simulating extension wake & download
    └── run_harness.ps1     # Automated test script for CI and local verification
```

---

## 3. Communication Protocol Specification

### 3.1 Port & Addressing
* **Port**: `14370` (Loopback only: `127.0.0.1:14370` / `localhost:14370`)
* **Transport**: WebSocket for live state streaming (`ws://127.0.0.1:14370`) and HTTP GET/OPTIONS for token negotiation (`http://127.0.0.1:14370/token`).

### 3.2 Authentication & Token Exchange
1. Desktop creates or reads a 64-character cryptographically random token at `%APPDATA%\EDM\token.txt`.
2. Extension requests token via:
   - Primary: `edm-nmh` Native Messaging message `{ "type": "get_token" }`.
   - Secondary: HTTP probe `GET /token` with headers `X-EDM-Client: extension` and `X-EDM-Extension-Id: <ID>`.
3. First WebSocket message MUST be `hello { token: "..." }` within 5 seconds; failure drops the socket immediately.

### 3.3 Wake & Recovery Flow
1. Extension intercepts download.
2. If WebSocket is closed, extension sends `{ "type": "wake", "show_window": false }` to `edm-nmh`.
3. `edm-nmh` launches `EDM.exe --background` using Windows `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP` flags (omitting `CREATE_NO_WINDOW` to avoid WebView2 GUI initialization lock).
4. `edm-nmh` probes port `14370` using exponential backoff (50ms -> 300ms) with a 20-second timeout.
5. On port readiness, `edm-nmh` replies with `{ "ok": true, "token": "..." }`.
6. Extension establishes WebSocket and transmits `{ "type": "create_task", ... }`.
7. If wake times out, browser fallback takes over and diagnostics registers `"last_wake_error"`.

---

## 4. Maintenance Rule
**Invariant**: If an issue occurs with extension communication, diagnose and fix it inside `core/handoff/`. Do NOT patch symptomatically in desktop components or extension UI.

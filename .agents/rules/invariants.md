# EDM Architectural Invariants & Anti-Regression Rules

> **MANDATORY FOR ALL CODING AGENTS**: When working on EDM, you MUST follow these invariants without exception to prevent recurring regressions. Full history and postmortems are cataloged in `docs/WORKFLOW_BUGS_LOG.md`.

## 1. The 4 Repeated Anti-Patterns (NEVER VIOLATE)

1. **`%LOCALAPPDATA%\EDM` IS THE INSTALL DIRECTORY**:
   - In Tauri v2 per-user installs, `$INSTDIR` is `%LOCALAPPDATA%\EDM`.
   - **NEVER** write `Delete` statements in NSIS hooks targeting `%LOCALAPPDATA%\EDM\*.exe`.
   - **NEVER** skip `%LOCALAPPDATA%\EDM` in NMH (`find_edm_exe()` or `repair_registration()`).

2. **WEBVIEW2LOADER.DLL ANTI-POISONING**:
   - **NEVER** trust file existence alone. Always check `file_size > 1000 bytes` (or `len() > 1000`).
   - A 0-byte placeholder (`b""`) will poison the release installer, causing Windows PE loader to crash with `WebView2Loader.dll was not found` or NTSTATUS `0xC000007B`.
   - `build.rs` can write a fallback placeholder only for cargo clippy on clean runners, but `build-release.ps1` MUST verify genuine size > 1000 bytes before packaging.

3. **BIDIRECTIONAL BINARY CO-LOCATION**:
   - Cargo builds `edm-desktop-app.exe`. Shortcuts link to `edm-desktop-app.exe`. Users and protocols look for `EDM.exe`.
   - Both `edm-desktop-app.exe` AND `EDM.exe` MUST exist in `$INSTDIR`.
   - `hooks.nsh` in `POSTINSTALL` must maintain both copies via `CopyFiles`.

4. **POSTINSTALL HOOKS ARE STRICTLY ADDITIVE**:
   - `NSIS_HOOK_POSTINSTALL` executes *after* payloads are unpacked into `$INSTDIR`.
   - **NEVER** execute `Delete` on application binaries or core assets in `POSTINSTALL`.

## 2. Invariant Verification Pre-Flight

Before completing any task, creating a commit, or building a release:
- Always run `powershell -ExecutionPolicy Bypass -File .\scripts\verify-invariants.ps1`.
- If any invariant fails, fix the code immediately; never bypass the validator.

## 3. Workflow Bug Logging Protocol

Whenever you identify or resolve a bug or regression:
- Append a structured incident entry to `docs/WORKFLOW_BUGS_LOG.md` categorized as `Critical`, `Major`, or `Minor`.
- Document:
  1. Incident ID & Title
  2. Symptoms / Error Messages
  3. What was forgotten / missed
  4. Root cause and technical diagnosis
  5. The surgical fix applied

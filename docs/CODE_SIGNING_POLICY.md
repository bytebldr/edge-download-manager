# Code Signing Policy

Free code signing provided by [**SignPath.org**](https://signpath.org/), using a certificate by [**SignPath Foundation**](https://signpath.foundation/).

---

## Scope

This policy applies to all official Windows binary releases of **Evo Download Manager (EDM)** published from `https://github.com/bytebldr/evo-download-manager`.

Only **first-party EDM binaries** built from source in GitHub Actions are submitted for signing:
- `EDM.exe` (Desktop application)
- `edm-nmh.exe` (Native Messaging Host)
- `EDM_*_x64-setup.exe` (Windows NSIS installer)

Third-party binaries (such as `WebView2Loader.dll`) are never re-signed.

---

## Build & Approval Controls

- **Trusted Build Environment**: All signed artifacts are built exclusively by the official GitHub Actions workflow (`.github/workflows/ci.yml`) from tagged releases (`v*`) or the protected `main` branch.
- **Committers & Reviewers**: Code changes are authored and reviewed by the `bytebldr` project maintainers.
- **Privacy**: See [PRIVACY.md](../PRIVACY.md). Evo Download Manager collects zero telemetry and performs no background tracking.

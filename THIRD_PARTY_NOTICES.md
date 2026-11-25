# Third-Party Notices

Evo Download Manager (EDM) is licensed under the **GNU General Public License v3.0 (`GPL-3.0-only`)** (see [LICENSE](LICENSE)).  
This file provides attribution, license notices, and GPLv3 compliance clarifications for third-party components and templates used by EDM.

---

## 1. Microsoft Edge WebView2 Redistributable (`WebView2Loader.dll`)

Evo Download Manager uses the Microsoft Edge WebView2 Runtime (and `WebView2Loader.dll`) on Windows to render the desktop user interface via Tauri.

- **Component**: Microsoft Edge WebView2 Redistributable / `WebView2Loader.dll`
- **Copyright**: © Microsoft Corporation
- **License**: Proprietary — Microsoft Edge WebView2 Redistributable License

### GPLv3 System Library & Additional Permission Clarification

1. **System Library (GPLv3 Section 1)**: The Microsoft Edge WebView2 Runtime is a standard operating-system platform component on Windows 10/11 and qualifies as a "System Library" under Section 1 of the GNU General Public License v3.0.
2. **Additional Permission (GPLv3 Section 7)**: To the extent `WebView2Loader.dll` is distributed alongside or linked with Evo Download Manager, the copyright holders of Evo Download Manager grant an **Additional Permission** under GPLv3 Section 7 permitting the combination, linking, and distribution of Evo Download Manager with the Microsoft Edge WebView2 Runtime and `WebView2Loader.dll`. You may remove this Additional Permission when distributing modified versions if you also remove the dependency on WebView2.

---

## 2. Tauri Windows NSIS Installer Template

The Windows installer script at `apps/desktop/src-tauri/windows/installer.nsi` is derived from Tauri's upstream NSIS installer template:

- **Copyright**: Copyright 2019–2024 Tauri Programme within The Commons Conservancy
- **License**: Dual-licensed under `Apache-2.0` OR `MIT`

---

## 3. Open-Source Rust & npm Dependencies

Evo Download Manager incorporates open-source packages from the Rust (`crates.io`) and JavaScript (`npm`) ecosystems, including:

### Key Rust Crates
- `tauri`, `tauri-build`, `tauri-plugin-single-instance`, `tauri-plugin-autostart` — `Apache-2.0 OR MIT`
- `tokio`, `axum`, `rusqlite`, `tracing`, `tracing-subscriber` — `MIT`
- `serde`, `serde_json`, `reqwest`, `thiserror`, `uuid`, `url`, `bytes`, `anyhow`, `sha2`, `sha1`, `md-5`, `hex`, `windows-sys`, `windows-registry` — `Apache-2.0 OR MIT`
- `librqbit` — `Apache-2.0`
- `subtle`, `chrono` — `BSD-3-Clause` / `Apache-2.0 OR MIT`

### Key npm Packages
- `react`, `react-dom`, `zustand`, `motion`, `recharts`, `tailwindcss`, `vite`, `clsx`, `tailwind-merge`, `esbuild` — `MIT`
- `lucide-react` — `ISC`
- `typescript` — `Apache-2.0`
- `@tauri-apps/api`, `@tauri-apps/cli` — `Apache-2.0 OR MIT`

Full license texts for individual crates and npm packages are available in their respective upstream repositories and package manifests.

# Contributing to Edge Download Manager

Thank you for your interest in contributing to **Edge Download Manager (EDM)**! We welcome contributions from developers of all skill levels to help build the fastest, most reliable open-source download manager for Windows.

---

## Code of Conduct

All contributors and maintainers are expected to adhere to our [Code of Conduct](CODE_OF_CONDUCT.md). Please read it before participating in discussions or submitting pull requests.

---

## Development Prerequisites

To build and run Edge Download Manager locally, you will need:

1. **Rust Toolchain**: Stable channel (`rustc 1.80+` or newer, `cargo`) with target `x86_64-pc-windows-msvc`.
   ```bash
   rustup default stable
   rustup component add clippy rustfmt
   ```
2. **Node.js**: Version 20 LTS or 22 LTS with `npm`.
3. **C++ Build Tools**: Visual Studio 2022 C++ Build Tools (Windows SDK and MSVC compiler for Tauri).
4. **WebView2 Runtime**: Included by default in Windows 10 and Windows 11.

---

## Building from Source

### 1. Clone the Repository
```bash
git clone https://github.com/bytebldr/edge-download-manager.git
cd edge-download-manager
```

### 2. Build the Browser Companion Extension
```bash
cd extension
npm install
npm run build
cd ..
```

### 3. Build the Desktop Application UI
```bash
cd apps/desktop
npm install
npm run build
cd ../..
```

### 4. Run Workspace Verification & Tests
Before making changes, verify that all existing workspace tests pass:
```bash
cargo test --workspace
powershell -ExecutionPolicy Bypass -File .\scripts\verify-invariants.ps1
```

### 5. Launch in Development Mode
To launch the live Tauri v2 desktop application with Hot Module Reloading (HMR):
```bash
cd apps/desktop
npm run tauri dev
```

---

## Code Style & Architectural Invariants

Contributors must maintain EDM's core engineering invariants:

1. **Defensive Programming**: Validate all external inputs with strict schemas before allocation or processing.
2. **Zero-Telemetry Invariant**: Never add unsolicited telemetry, remote tracking, or analytics SDKs.
3. **Workspace Integrity**: Ensure all crates (`packages/engine`, `packages/store`, `packages/ipc`, `apps/desktop`) maintain synchronized versions and compile warning-free under `cargo clippy`.
4. **Pre-Flight Invariants**: Always run `.\scripts\verify-invariants.ps1` before committing.
5. **Code Formatting**:
   - Rust: `cargo fmt`
   - TypeScript/React: Standard ESLint & Prettier styling.

---

## Submitting a Pull Request (PR)

1. **Branch Naming**:
   - `fix/<short-description>` for bug fixes
   - `feat/<short-description>` for new capabilities
   - `docs/<short-description>` for documentation improvements
2. **Commit Messages**: Follow standard Conventional Commits format (`feat: ...`, `fix: ...`, `docs: ...`, `refactor: ...`).
3. **Tests**: Include unit or integration tests for any new or modified feature logic.
4. **PR Description**: Detail what changed, why the change is necessary, and test results demonstrating functionality.

All pull requests must pass the automated GitHub Actions CI pipeline (`rust-check`, `frontend-check`, `verify-invariants.ps1`) before merging.

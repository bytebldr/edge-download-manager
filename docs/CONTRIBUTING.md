# Contributing to Evo Download Manager

Thank you for your interest in contributing to **Evo Download Manager (EDM)** (`https://github.com/bytebldr/evo-download-manager`)!

---

## Prerequisites

- **Rust** (stable toolchain, `x86_64-pc-windows-msvc`)
- **Node.js** (20+ LTS / 22)
- **Visual Studio C++ Build Tools** (Windows SDK)

---

## Local Development

```bash
git clone https://github.com/bytebldr/evo-download-manager.git
cd evo-download-manager

# Build browser extension
cd extension && npm install && npm run build && cd ..

# Build desktop UI & run Rust tests
cd apps/desktop && npm install && npm run build && cd ../..
cargo test --workspace
```

---

## Pull Request Guidelines

- Keep changes focused and minimal.
- Run `cargo clippy --workspace --all-targets` and `cargo test --workspace` before submitting.
- Do not introduce telemetry, tracking, or unrequested third-party dependencies.
- All contributions are licensed under `GPL-3.0-only` (see [LICENSE](../LICENSE)).

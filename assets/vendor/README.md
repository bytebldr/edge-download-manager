# EDM Vendor Static Assets Vault

This directory stores immutable, version-controlled runtime dependencies and manifest templates required for EDM packaging and self-healing.

## Directory Structure
- `win-x64/WebView2Loader.dll`: Microsoft Edge WebView2 redistributable loader for 64-bit Windows (`160,880 bytes`).
- `win-x64/WebView2Loader.dll.sha256`: Cryptographic SHA-256 integrity hash (`86545b66cdb0603bc26b626fb9ad610cb6e71f28d468f5ea66df23b03dda96d5`).
- `manifests/com.edm.nmh.json`: Canonical Native Messaging Host manifest template for Chromium browsers (Edge, Chrome, Brave).
- `manifests/com.edm.nmh.firefox.json`: Canonical Native Messaging Host manifest template for Mozilla Firefox.

## Why this directory exists:
1. Prevents packaging dependency on volatile compiler output directories (`target/`).
2. Enables clean CI builds and local compilation without chicken-and-egg placeholder scripts.
3. Serves as the source of truth for runtime self-healing routines.

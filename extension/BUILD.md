# Building the EDM Browser Extension (AMO Source Review Guide)

## Environment Requirements
- **Node.js**: v20.x or newer (LTS recommended)
- **npm**: v10.x or newer
- **OS**: Windows, macOS, or Linux (no native compilation required; pure JS/TS bundle via `esbuild`)

## Reproducible Build Steps
1. Enter the `extension/` directory:
   ```bash
   cd extension
   ```
2. Install exact pinned devDependencies (`esbuild` and `typescript`):
   ```bash
   npm ci
   ```
3. Run the build script (bundles TypeScript sources with `minify: false` for full readability):
   ```bash
   npm run build
   ```

## Build Outputs
- `dist/firefox/` — Unpacked Firefox MV3 extension directory (`manifest.json` from `manifest.firefox.json`, unminified `.js` bundles, HTML/CSS/PNG assets).
- `dist/edm-firefox.xpi` — Packaged Firefox add-on archive built from `dist/firefox/`.
- `dist/edm-firefox-source.zip` — Clean source archive containing `src/`, `manifest.firefox.json`, `package.json`, `package-lock.json`, `build.js`, and `BUILD.md`.
- `dist/chromium/` — Unpacked Chromium MV3 extension directory.
- `dist/edm-extension-chromium.zip` — Store-compliant Chromium package (strips `key` and YouTube main-world helper).

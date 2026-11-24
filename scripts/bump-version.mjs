import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, "..");

// 1. Determine target version
const tauriConfPath = path.join(rootDir, "apps", "desktop", "src-tauri", "tauri.conf.json");
const tauriConf = JSON.parse(fs.readFileSync(tauriConfPath, "utf-8"));

const argVersion = process.argv[2];

if (argVersion === "--check") {
  const version = tauriConf.version;
  console.log(`[bump-version] Checking version parity across all files (expected: ${version})...`);
  let hasMismatch = false;

  const cargoTomlPath = path.join(rootDir, "Cargo.toml");
  const cargoToml = fs.readFileSync(cargoTomlPath, "utf-8");
  const cargoMatch = cargoToml.match(/\[workspace\.package\][\s\S]*?version\s*=\s*"([^"]+)"/);
  if (!cargoMatch || cargoMatch[1] !== version) {
    console.error(`  ✗ Cargo.toml mismatch: expected ${version}, found ${cargoMatch ? cargoMatch[1] : 'none'}`);
    hasMismatch = true;
  } else {
    console.log(`  ✓ Cargo.toml matches (${version})`);
  }

  const desktopCargoPath = path.join(rootDir, "apps", "desktop", "src-tauri", "Cargo.toml");
  const desktopCargo = fs.readFileSync(desktopCargoPath, "utf-8");
  if (!desktopCargo.includes("version.workspace = true")) {
    console.error(`  ✗ apps/desktop/src-tauri/Cargo.toml must have version.workspace = true`);
    hasMismatch = true;
  } else {
    console.log(`  ✓ apps/desktop/src-tauri/Cargo.toml inherits workspace version`);
  }

  const checkJson = (filePath, name) => {
    if (!fs.existsSync(filePath)) return;
    const data = JSON.parse(fs.readFileSync(filePath, "utf-8"));
    if (data.version !== version) {
      console.error(`  ✗ ${name} mismatch: expected ${version}, found ${data.version}`);
      hasMismatch = true;
    } else {
      console.log(`  ✓ ${name} matches (${version})`);
    }
  };

  checkJson(path.join(rootDir, "apps", "desktop", "package.json"), "apps/desktop/package.json");
  checkJson(path.join(rootDir, "extension", "package.json"), "extension/package.json");
  checkJson(path.join(rootDir, "extension", "manifest.json"), "extension/manifest.json");
  checkJson(path.join(rootDir, "extension", "manifest.chromium.json"), "extension/manifest.chromium.json");
  checkJson(path.join(rootDir, "extension", "manifest.firefox.json"), "extension/manifest.firefox.json");

  if (hasMismatch) {
    console.error(`[bump-version] Version parity check FAILED!`);
    process.exit(1);
  } else {
    console.log(`[bump-version] All versions in sync at v${version}`);
    process.exit(0);
  }
}

const targetVersion = argVersion ? argVersion.replace(/^v/, "") : tauriConf.version;

if (!targetVersion || !/^\d+\.\d+\.\d+/.test(targetVersion)) {
  console.error(`Invalid version format: ${targetVersion}. Expected semver (e.g. 0.0.14)`);
  process.exit(1);
}

console.log(`[bump-version] Syncing version to: ${targetVersion}`);

// 2. apps/desktop/src-tauri/tauri.conf.json
tauriConf.version = targetVersion;
if (tauriConf.app?.windows?.[0]?.title) {
  tauriConf.app.windows[0].title = `EDM — Edge Download Manager v${targetVersion}`;
}
fs.writeFileSync(tauriConfPath, JSON.stringify(tauriConf, null, 2) + "\n", "utf-8");
console.log(`  ✓ Updated apps/desktop/src-tauri/tauri.conf.json`);

// 3. Root Cargo.toml
const cargoTomlPath = path.join(rootDir, "Cargo.toml");
let cargoToml = fs.readFileSync(cargoTomlPath, "utf-8");
cargoToml = cargoToml.replace(
  /(\[workspace\.package\][\s\S]*?version\s*=\s*")[^"]+(")/,
  `$1${targetVersion}$2`
);
fs.writeFileSync(cargoTomlPath, cargoToml, "utf-8");
console.log(`  ✓ Updated Cargo.toml`);

// 3b. apps/desktop/src-tauri/Cargo.toml (must inherit workspace version)
const desktopCargoPath = path.join(rootDir, "apps", "desktop", "src-tauri", "Cargo.toml");
let desktopCargo = fs.readFileSync(desktopCargoPath, "utf-8");
if (!desktopCargo.includes("version.workspace = true")) {
  desktopCargo = desktopCargo.replace(/version\s*=\s*"[^"]+"/, "version.workspace = true");
  fs.writeFileSync(desktopCargoPath, desktopCargo, "utf-8");
  console.log(`  ✓ Enforced version.workspace = true in apps/desktop/src-tauri/Cargo.toml`);
}

// 4. apps/desktop/package.json
const desktopPkgPath = path.join(rootDir, "apps", "desktop", "package.json");
const desktopPkg = JSON.parse(fs.readFileSync(desktopPkgPath, "utf-8"));
desktopPkg.version = targetVersion;
fs.writeFileSync(desktopPkgPath, JSON.stringify(desktopPkg, null, 2) + "\n", "utf-8");
console.log(`  ✓ Updated apps/desktop/package.json`);

// 5. extension/package.json
const extPkgPath = path.join(rootDir, "extension", "package.json");
const extPkg = JSON.parse(fs.readFileSync(extPkgPath, "utf-8"));
extPkg.version = targetVersion;
fs.writeFileSync(extPkgPath, JSON.stringify(extPkg, null, 2) + "\n", "utf-8");
console.log(`  ✓ Updated extension/package.json`);

// 6. extension/manifest.chromium.json
const extChromiumPath = path.join(rootDir, "extension", "manifest.chromium.json");
if (fs.existsSync(extChromiumPath)) {
  const extChromium = JSON.parse(fs.readFileSync(extChromiumPath, "utf-8"));
  extChromium.version = targetVersion;
  fs.writeFileSync(extChromiumPath, JSON.stringify(extChromium, null, 2) + "\n", "utf-8");
  console.log(`  ✓ Updated extension/manifest.chromium.json`);
}

// 7. extension/manifest.firefox.json
const extFirefoxPath = path.join(rootDir, "extension", "manifest.firefox.json");
if (fs.existsSync(extFirefoxPath)) {
  const extFirefox = JSON.parse(fs.readFileSync(extFirefoxPath, "utf-8"));
  extFirefox.version = targetVersion;
  fs.writeFileSync(extFirefoxPath, JSON.stringify(extFirefox, null, 2) + "\n", "utf-8");
  console.log(`  ✓ Updated extension/manifest.firefox.json`);
}

// 8. extension/manifest.json
const extManifestPath = path.join(rootDir, "extension", "manifest.json");
if (fs.existsSync(extManifestPath)) {
  const extManifest = JSON.parse(fs.readFileSync(extManifestPath, "utf-8"));
  extManifest.version = targetVersion;
  fs.writeFileSync(extManifestPath, JSON.stringify(extManifest, null, 2) + "\n", "utf-8");
  console.log(`  ✓ Updated extension/manifest.json`);
}

// 9. hooks.nsh registry Version string
const hooksNshPath = path.join(rootDir, "apps", "desktop", "src-tauri", "windows", "hooks.nsh");
if (fs.existsSync(hooksNshPath)) {
  let hooksNsh = fs.readFileSync(hooksNshPath, "utf-8");
  hooksNsh = hooksNsh.replace(
    /(WriteRegStr HKCU "Software\\EDM" "Version" ")[^"]+(")/,
    `$1${targetVersion}$2`
  );
  fs.writeFileSync(hooksNshPath, hooksNsh, "utf-8");
  console.log(`  ✓ Updated apps/desktop/src-tauri/windows/hooks.nsh`);
}

console.log(`[bump-version] Version synchronization complete: v${targetVersion}`);

import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, "..");

// ---------------------------------------------------------
// TEST 1: Version Parity Check (All 7 targets match Cargo.toml)
// ---------------------------------------------------------
test("1. Version synchronization parity across all targets", () => {
  // 1. Root Cargo.toml
  const cargoToml = fs.readFileSync(path.join(rootDir, "Cargo.toml"), "utf8");
  const cargoVerMatch = cargoToml.match(/\[workspace\.package\][\s\S]*?version\s*=\s*"([^"]+)"/);
  assert.ok(cargoVerMatch, "Root Cargo.toml version match");
  const expected = cargoVerMatch[1];

  // 2. Tauri conf
  const tauriConf = JSON.parse(
    fs.readFileSync(
      path.join(rootDir, "apps", "desktop", "src-tauri", "tauri.conf.json"),
      "utf8"
    )
  );
  assert.equal(tauriConf.version, expected, "tauri.conf.json version");

  // 3. Desktop package.json
  const desktopPkg = JSON.parse(
    fs.readFileSync(path.join(rootDir, "apps", "desktop", "package.json"), "utf8")
  );
  assert.equal(desktopPkg.version, expected, "apps/desktop/package.json version");

  // 4. Extension package.json
  const extPkg = JSON.parse(
    fs.readFileSync(path.join(rootDir, "extension", "package.json"), "utf8")
  );
  assert.equal(extPkg.version, expected, "extension/package.json version");

  // 5. Extension manifest.json
  const extManifest = JSON.parse(
    fs.readFileSync(path.join(rootDir, "extension", "manifest.json"), "utf8")
  );
  assert.equal(extManifest.version, expected, "extension/manifest.json version");

  // 6. Extension manifest.chromium.json
  const extChromium = JSON.parse(
    fs.readFileSync(path.join(rootDir, "extension", "manifest.chromium.json"), "utf8")
  );
  assert.equal(extChromium.version, expected, "extension/manifest.chromium.json version");

  // 7. Extension manifest.firefox.json
  const extFirefox = JSON.parse(
    fs.readFileSync(path.join(rootDir, "extension", "manifest.firefox.json"), "utf8")
  );
  assert.equal(extFirefox.version, expected, "extension/manifest.firefox.json version");
});

// ---------------------------------------------------------
// TEST 2: Extension Name is strictly "EDM"
// ---------------------------------------------------------
test("2. Extension Name is strictly 'EDM' (Bug 9)", () => {
  const manifests = [
    path.join(rootDir, "extension", "manifest.json"),
    path.join(rootDir, "extension", "manifest.chromium.json"),
    path.join(rootDir, "extension", "manifest.firefox.json"),
  ];

  for (const m of manifests) {
    const content = JSON.parse(fs.readFileSync(m, "utf8"));
    assert.equal(content.name, "EDM", `${path.basename(m)} name must be strictly 'EDM'`);
    assert.ok(
      content.name !== "Edge Download Manager" && !content.name.includes("Edge Download Manager"),
      `${path.basename(m)} name must not be or include 'Edge Download Manager'`
    );
    assert.ok(
      content.name !== "Evo Download Manager" && !content.name.includes("Evo Download Manager"),
      `${path.basename(m)} name must not be or include 'Evo Download Manager'`
    );
  }
});

// ---------------------------------------------------------
// TEST 3: PRO text removed from Settings About section (Bug 3)
// ---------------------------------------------------------
test("3. 'PRO ' text removed from Settings About section badge (Bug 3)", () => {
  const settingsPage = fs.readFileSync(
    path.join(rootDir, "apps", "desktop", "src", "components", "SettingsPage.tsx"),
    "utf8"
  );
  assert.ok(
    !settingsPage.includes("PRO v{appVersion}"),
    "SettingsPage.tsx must not contain 'PRO v{appVersion}'"
  );
  assert.ok(
    settingsPage.includes("v{appVersion}"),
    "SettingsPage.tsx should display 'v{appVersion}' cleanly"
  );
});

// ---------------------------------------------------------
// TEST 4: 'OTA' text removed across entire application (Bug 5)
// ---------------------------------------------------------
test("4. 'OTA' text removed across entire application UI (Bug 5)", () => {
  const settingsPage = fs.readFileSync(
    path.join(rootDir, "apps", "desktop", "src", "components", "SettingsPage.tsx"),
    "utf8"
  );
  assert.ok(
    !settingsPage.includes("Update Now (OTA)"),
    "SettingsPage.tsx must not contain 'Update Now (OTA)'"
  );
  assert.ok(
    !settingsPage.includes("Software Updates & OTA Pipeline"),
    "SettingsPage.tsx must not contain 'Software Updates & OTA Pipeline'"
  );
  assert.ok(
    settingsPage.includes("<span>Update Now</span>"),
    "SettingsPage.tsx should have 'Update Now'"
  );
  assert.ok(
    settingsPage.includes("Software Updates & Pipeline"),
    "SettingsPage.tsx should have 'Software Updates & Pipeline'"
  );
});

// ---------------------------------------------------------
// TEST 5: Dedicated Reinstall & Repair UI exists (Bug 4)
// ---------------------------------------------------------
test("5. Dedicated Reinstall & Repair UI (#repair / --repair) (Bug 4)", () => {
  const repairWizardBarrelPath = path.join(
    rootDir,
    "apps",
    "desktop",
    "src",
    "components",
    "RepairWizard.tsx"
  );
  assert.ok(fs.existsSync(repairWizardBarrelPath), "components/RepairWizard.tsx must exist");

  const repairWizardPath = path.join(
    rootDir,
    "apps",
    "desktop",
    "src",
    "lifecycle",
    "wizards",
    "RepairWizard.tsx"
  );
  assert.ok(fs.existsSync(repairWizardPath), "lifecycle/wizards/RepairWizard.tsx must exist");

  const repairContent = fs.readFileSync(repairWizardPath, "utf8");
  assert.ok(
    repairContent.includes("Repair browser integration & Native Messaging Host"),
    "Must offer browser integration repair"
  );
  assert.ok(
    repairContent.includes("Restore missing Desktop and Start Menu shortcuts"),
    "Must offer shortcuts repair"
  );
  assert.ok(
    repairContent.includes("Verify and restore core binaries"),
    "Must offer binary integrity check"
  );
  assert.ok(
    repairContent.includes("Preserves all downloads, queues, and user settings"),
    "Must have explicit non-destructive badge"
  );

  const lifecycleAppTsx = fs.readFileSync(
    path.join(rootDir, "apps", "desktop", "src", "lifecycle", "LifecycleApp.tsx"),
    "utf8"
  );
  assert.ok(lifecycleAppTsx.includes("RepairWizard"), "LifecycleApp.tsx must mount RepairWizard");
  assert.ok(lifecycleAppTsx.includes("#repair"), "LifecycleApp.tsx must support #repair route");

  const mainTsx = fs.readFileSync(
    path.join(rootDir, "apps", "desktop", "src", "main.tsx"),
    "utf8"
  );
  assert.ok(mainTsx.includes("LifecycleApp"), "main.tsx must mount LifecycleApp");
  assert.ok(mainTsx.includes("resolveLifecycleRoute"), "main.tsx must resolve lifecycle routes");
});

// ---------------------------------------------------------
// TEST 6: Torrent Upload Telemetry & Elapsed / Time Left (Bug 2)
// ---------------------------------------------------------
test("6. Torrent Upload Telemetry and Elapsed time support (Bug 2)", () => {
  const btPack = fs.readFileSync(
    path.join(rootDir, "core", "engine", "src", "packs", "bittorrent.rs"),
    "utf8"
  );
  assert.ok(
    btPack.includes("upload_throughput_bps"),
    "bittorrent.rs must populate upload_throughput_bps"
  );

  const speedChart = fs.readFileSync(
    path.join(rootDir, "apps", "desktop", "src", "components", "SpeedChartInteractive.tsx"),
    "utf8"
  );
  assert.ok(
    speedChart.includes("uploadHistory"),
    "SpeedChartInteractive must accept uploadHistory"
  );

  const drawer = fs.readFileSync(
    path.join(rootDir, "apps", "desktop", "src", "components", "DetailsInspectorDrawer.tsx"),
    "utf8"
  );
  assert.ok(
    drawer.includes("uploadHistory={download.uploadHistory}"),
    "DetailsInspectorDrawer must pass uploadHistory"
  );
  assert.ok(
    drawer.includes("elapsedSeconds={download.elapsedSeconds || 0}"),
    "DetailsInspectorDrawer must pass real elapsedSeconds"
  );
});

// ---------------------------------------------------------
// TEST 7: File Collision and Concurrency Limits (Bug 6 & Bug 8)
// ---------------------------------------------------------
test("7. File Collision and Concurrency Policy (Bug 6 & Bug 8)", () => {
  const dm = fs.readFileSync(
    path.join(rootDir, "apps", "desktop", "src-tauri", "src", "services", "download_manager.rs"),
    "utf8"
  );
  assert.ok(
    dm.includes("filenameConflictPolicy"),
    "download_manager.rs must check filenameConflictPolicy"
  );
  assert.ok(
    dm.includes("maxActiveDownloads"),
    "download_manager.rs must enforce maxActiveDownloads concurrency limit"
  );
  assert.ok(
    dm.includes("DownloadState::Queued"),
    "download_manager.rs must support queueing when concurrency limit is hit"
  );
});

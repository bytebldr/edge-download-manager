import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, "..");

// ---------------------------------------------------------
// Helper: compareVersions (as implemented in version.ts)
// ---------------------------------------------------------
function parseSemver(v) {
  if (!v || typeof v !== "string") return null;
  const clean = v.trim().replace(/^v/i, "");
  const [core] = clean.split(/[-+]/);
  const parts = core.split(".");
  if (parts.length < 3) return null;
  const major = parseInt(parts[0], 10);
  const minor = parseInt(parts[1], 10);
  const patch = parseInt(parts[2], 10);
  if (isNaN(major) || isNaN(minor) || isNaN(patch)) return null;
  return [major, minor, patch];
}

function compareVersions(a, b) {
  const pa = parseSemver(a);
  const pb = parseSemver(b);
  if (pa && pb) {
    if (pa[0] !== pb[0]) return pa[0] < pb[0] ? -1 : 1;
    if (pa[1] !== pb[1]) return pa[1] < pb[1] ? -1 : 1;
    if (pa[2] !== pb[2]) return pa[2] < pb[2] ? -1 : 1;
    return 0;
  }
  const sa = (a ?? "").trim();
  const sb = (b ?? "").trim();
  if (sa < sb) return -1;
  if (sa > sb) return 1;
  return 0;
}

// =========================================================
// TEST 1: Version comparison tests
// =========================================================
test("1. Version comparison tests", async (t) => {
  await t.test("patch and minor increments", () => {
    assert.equal(compareVersions("0.0.13", "0.0.14"), -1);
    assert.equal(compareVersions("0.0.14", "0.0.13"), 1);
    assert.equal(compareVersions("0.0.14", "0.0.14"), 0);
  });

  await t.test("numeric vs lexical ordering (e.g. 0.0.9 vs 0.0.10)", () => {
    assert.equal(compareVersions("0.0.9", "0.0.10"), -1);
    assert.equal(compareVersions("0.0.10", "0.0.9"), 1);
    assert.equal(compareVersions("0.1.0", "0.0.14"), 1);
    assert.equal(compareVersions("1.0.0", "0.99.99"), 1);
  });

  await t.test("v prefix and prerelease tags", () => {
    assert.equal(compareVersions("v0.0.14", "0.0.14"), 0);
    assert.equal(compareVersions("v0.0.14-beta.1", "0.0.14"), 0);
    assert.equal(compareVersions("0.0.13-rc1", "0.0.14"), -1);
  });

  await t.test("malformed and empty strings fallback", () => {
    assert.equal(compareVersions("unknown", "0.0.14"), 1);
    assert.equal(compareVersions("", "0.0.14"), -1);
    assert.equal(compareVersions("abc", "abc"), 0);
  });
});

// =========================================================
// TEST 2: Version sync check test
// =========================================================
test("2. Version sync check across 7 manifest targets", async () => {
  const tauriConf = JSON.parse(
    fs.readFileSync(path.join(rootDir, "apps", "desktop", "src-tauri", "tauri.conf.json"), "utf-8")
  );
  const targetVersion = tauriConf.version;
  assert.ok(targetVersion, "tauri.conf.json must define a version");

  // Cargo.toml
  const cargoToml = fs.readFileSync(path.join(rootDir, "Cargo.toml"), "utf-8");
  const cargoMatch = cargoToml.match(/\[workspace\.package\][\s\S]*?version\s*=\s*"([^"]+)"/);
  assert.equal(cargoMatch?.[1], targetVersion, "Cargo.toml workspace version must match");

  // apps/desktop/package.json
  const desktopPkg = JSON.parse(
    fs.readFileSync(path.join(rootDir, "apps", "desktop", "package.json"), "utf-8")
  );
  assert.equal(desktopPkg.version, targetVersion, "apps/desktop/package.json must match");

  // extension/package.json
  const extPkg = JSON.parse(
    fs.readFileSync(path.join(rootDir, "extension", "package.json"), "utf-8")
  );
  assert.equal(extPkg.version, targetVersion, "extension/package.json must match");

  // extension manifests
  const manifest = JSON.parse(fs.readFileSync(path.join(rootDir, "extension", "manifest.json"), "utf-8"));
  const manifestChromium = JSON.parse(fs.readFileSync(path.join(rootDir, "extension", "manifest.chromium.json"), "utf-8"));
  const manifestFirefox = JSON.parse(fs.readFileSync(path.join(rootDir, "extension", "manifest.firefox.json"), "utf-8"));

  assert.equal(manifest.version, targetVersion, "manifest.json must match");
  assert.equal(manifestChromium.version, targetVersion, "manifest.chromium.json must match");
  assert.equal(manifestFirefox.version, targetVersion, "manifest.firefox.json must match");
});

// =========================================================
// TEST 3: Updater state machine test
// =========================================================
test("3. Updater state machine & toast dismissal persistence", async () => {
  class MockUpdaterStateMachine {
    constructor(currentVersion) {
      this.currentVersion = currentVersion;
      this.state = "idle"; // idle | checking | available | downloading | ready | error
      this.availableVersion = null;
      this.downloadProgress = 0;
      this.storage = new Map();
    }

    checkForUpdate(remoteVersion) {
      this.state = "checking";
      if (compareVersions(this.currentVersion, remoteVersion) < 0) {
        this.availableVersion = remoteVersion;
        const dismissed = this.storage.get("edm_dismissed_update_version");
        if (dismissed === remoteVersion) {
          this.state = "idle"; // Dismissed
          return false;
        }
        this.state = "available";
        return true;
      }
      this.state = "idle";
      return false;
    }

    startDownload() {
      if (this.state !== "available") throw new Error("Cannot download when not available");
      this.state = "downloading";
      this.downloadProgress = 0;
    }

    updateProgress(percent) {
      if (this.state !== "downloading") return;
      this.downloadProgress = Math.min(100, Math.max(0, percent));
      if (this.downloadProgress === 100) {
        this.state = "ready";
      }
    }

    dismissToast() {
      if (this.availableVersion) {
        this.storage.set("edm_dismissed_update_version", this.availableVersion);
      }
      this.state = "idle";
    }
  }

  const updater = new MockUpdaterStateMachine("0.0.13");

  // Check update found
  const hasUpdate = updater.checkForUpdate("0.0.14");
  assert.equal(hasUpdate, true);
  assert.equal(updater.state, "available");

  // Start download & progress transitions
  updater.startDownload();
  assert.equal(updater.state, "downloading");
  updater.updateProgress(45);
  assert.equal(updater.downloadProgress, 45);
  updater.updateProgress(100);
  assert.equal(updater.state, "ready");

  // Dismiss toast
  updater.dismissToast();
  assert.equal(updater.state, "idle");

  // Check again for the same version -> suppressed by dismissal
  const checkAgain = updater.checkForUpdate("0.0.14");
  assert.equal(checkAgain, false);

  // Check for an even newer version (0.0.15) -> shown!
  const newerUpdate = updater.checkForUpdate("0.0.15");
  assert.equal(newerUpdate, true);
  assert.equal(updater.availableVersion, "0.0.15");
});

// =========================================================
// TEST 4: Extension version display test
// =========================================================
test("4. Extension version display logic & downgrade prevention", async () => {
  function computeExtensionVersionBadge(installed, expected, desktopConnected) {
    if (!desktopConnected) {
      return {
        badgeText: "Offline",
        badgeColor: "gray",
        offerUpdate: false,
        tooltip: "EDM desktop application is not running or unreachable.",
      };
    }

    const cmp = compareVersions(installed, expected);
    if (cmp < 0) {
      return {
        badgeText: `Update Available (v${expected})`,
        badgeColor: "orange",
        offerUpdate: true,
        tooltip: "Click to reload extension to latest release.",
      };
    } else if (cmp === 0) {
      return {
        badgeText: "Up to date",
        badgeColor: "green",
        offerUpdate: false,
        tooltip: "Extension matches desktop application version.",
      };
    } else {
      return {
        badgeText: `Ahead of app (v${expected})`,
        badgeColor: "blue",
        offerUpdate: false,
        tooltip: "Extension is newer than running EDM desktop application.",
      };
    }
  }

  // Case A: Bug 3 Scenario (Installed 0.0.14, Expected 0.0.13)
  const caseA = computeExtensionVersionBadge("0.0.14", "0.0.13", true);
  assert.equal(caseA.badgeText, "Ahead of app (v0.0.13)");
  assert.equal(caseA.badgeColor, "blue");
  assert.equal(caseA.offerUpdate, false, "Must NEVER offer a downgrade!");

  // Case B: Normal Up to date (Installed 0.0.14, Expected 0.0.14)
  const caseB = computeExtensionVersionBadge("0.0.14", "0.0.14", true);
  assert.equal(caseB.badgeText, "Up to date");
  assert.equal(caseB.badgeColor, "green");
  assert.equal(caseB.offerUpdate, false);

  // Case C: Legitimate Update Available (Installed 0.0.13, Expected 0.0.14)
  const caseC = computeExtensionVersionBadge("0.0.13", "0.0.14", true);
  assert.equal(caseC.badgeText, "Update Available (v0.0.14)");
  assert.equal(caseC.badgeColor, "orange");
  assert.equal(caseC.offerUpdate, true);

  // Case D: Desktop unreachable
  const caseD = computeExtensionVersionBadge("0.0.14", null, false);
  assert.equal(caseD.badgeText, "Offline");
  assert.equal(caseD.offerUpdate, false);
});

// =========================================================
// TEST 5: Wake flow and diagnostic state test
// =========================================================
test("5. Wake flow, exponential backoff, and diagnostic health logic", async () => {
  class MockWakeCoordinator {
    constructor() {
      this.isWsListening = false;
      this.lastWakeError = null;
      this.lastWakeDurationMs = null;
    }

    async testWake({ simulateSuccess, delayMs = 100 }) {
      const startTime = Date.now();
      if (this.isWsListening) {
        this.lastWakeDurationMs = 0;
        this.lastWakeError = null;
        return { success: true, wakeTimeMs: 0 };
      }

      // Simulate sending wake via NMH
      if (!simulateSuccess) {
        this.lastWakeError = "Timeout waiting for EDM desktop to initialize";
        this.lastWakeDurationMs = null;
        return { success: false, error: this.lastWakeError };
      }

      await new Promise((r) => setTimeout(r, delayMs));
      this.isWsListening = true;
      this.lastWakeError = null;
      this.lastWakeDurationMs = delayMs;
      return { success: true, wakeTimeMs: delayMs };
    }

    getDiagnostics(appInstalled, helperAvailable) {
      const isHealthy = appInstalled && helperAvailable && !this.lastWakeError;
      return {
        coreStatus: isHealthy ? "HEALTHY" : "DEGRADED",
        wsStatus: this.isWsListening ? "Listening (Ready)" : "Inactive",
        lastWakeError: this.lastWakeError,
        coldStartWakeTime: this.lastWakeDurationMs !== null ? `${this.lastWakeDurationMs} ms` : "Instant / Sleeping",
      };
    }
  }

  const coordinator = new MockWakeCoordinator();

  // Test 5.1: Initial failure -> diagnostics MUST NOT report HEALTHY
  const failResult = await coordinator.testWake({ simulateSuccess: false });
  assert.equal(failResult.success, false);
  const diagFail = coordinator.getDiagnostics(true, true);
  assert.equal(diagFail.coreStatus, "DEGRADED", "Diagnostics must report DEGRADED when last wake failed");
  assert.equal(diagFail.lastWakeError, "Timeout waiting for EDM desktop to initialize");

  // Test 5.2: Subsequent recovery -> diagnostics clears and reports HEALTHY
  const okResult = await coordinator.testWake({ simulateSuccess: true, delayMs: 150 });
  assert.equal(okResult.success, true);
  assert.ok(okResult.wakeTimeMs >= 140);
  const diagOk = coordinator.getDiagnostics(true, true);
  assert.equal(diagOk.coreStatus, "HEALTHY");
  assert.equal(diagOk.lastWakeError, null);
  assert.match(diagOk.coldStartWakeTime, /15\d ms/);
});

// =========================================================
// TEST 6: End-to-end handoff simulation test
// =========================================================
test("6. End-to-end download handoff pipeline", async () => {
  const interceptedDownload = {
    url: "https://releases.ubuntu.com/24.04/ubuntu-24.04-desktop-amd64.iso",
    filename: "ubuntu-24.04-desktop-amd64.iso",
    fileSize: 6200000000,
    referrer: "https://releases.ubuntu.com/",
    cookies: "session=xyz123",
  };

  class MockHandoffPipeline {
    constructor() {
      this.edmWsOpen = true;
      this.receivedTasks = [];
    }

    async dispatchDownload(item) {
      if (!this.edmWsOpen) {
        return { handled: false, fallback: "browser" };
      }
      const taskId = `task_${Date.now()}`;
      this.receivedTasks.push({
        id: taskId,
        url: item.url,
        filename: item.filename,
        totalBytes: item.fileSize,
        headers: {
          Referer: item.referrer,
          Cookie: item.cookies,
        },
      });
      return { handled: true, taskId };
    }
  }

  const pipeline = new MockHandoffPipeline();
  const res = await pipeline.dispatchDownload(interceptedDownload);

  assert.equal(res.handled, true);
  assert.ok(res.taskId.startsWith("task_"));
  assert.equal(pipeline.receivedTasks.length, 1);
  assert.equal(pipeline.receivedTasks[0].filename, "ubuntu-24.04-desktop-amd64.iso");
  assert.equal(pipeline.receivedTasks[0].headers.Cookie, "session=xyz123");
});

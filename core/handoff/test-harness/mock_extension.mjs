#!/usr/bin/env node

/**
 * EDM Mock Extension Test Harness
 * Tests Native Messaging Host (edm-nmh.exe), Desktop Wake, and WebSocket/HTTP Token Server.
 */

import { spawn } from "node:child_process";
import http from "node:http";
import assert from "node:assert/strict";
import path from "node:path";
import fs from "node:fs";

const EXTENSION_ID = "ginpdimhcdbojbhemgjedghkeolbcmgj";
const LOCALAPPDATA = process.env.LOCALAPPDATA || path.join(process.env.USERPROFILE, "AppData", "Local");
const DEFAULT_NMH_PATH = path.join(LOCALAPPDATA, "EDM", "edm-nmh.exe");

const nmhPath = process.argv[2] || (fs.existsSync(DEFAULT_NMH_PATH) ? DEFAULT_NMH_PATH : path.resolve("../../target/release/edm-nmh.exe"));

console.log("=================================================");
console.log(" [EDM Handoff Harness] Testing Extension ↔ App IPC");
console.log(` NMH Binary: ${nmhPath}`);
console.log("=================================================\n");

function sendNativeMessage(proc, obj) {
  const json = JSON.stringify(obj);
  const lenBuf = Buffer.alloc(4);
  lenBuf.writeUInt32LE(Buffer.byteLength(json));
  proc.stdin.write(lenBuf);
  proc.stdin.write(json);
}

function readNativeMessage(proc, timeoutMs = 10000) {
  return new Promise((resolve, reject) => {
    let buf = Buffer.alloc(0);
    const timer = setTimeout(() => {
      cleanup();
      reject(new Error(`Timeout (${timeoutMs}ms) waiting for NMH response`));
    }, timeoutMs);

    function onData(chunk) {
      buf = Buffer.concat([buf, chunk]);
      if (buf.length >= 4) {
        const msgLen = buf.readUInt32LE(0);
        if (buf.length >= 4 + msgLen) {
          const raw = buf.subarray(4, 4 + msgLen).toString("utf-8");
          cleanup();
          try {
            resolve(JSON.parse(raw));
          } catch (e) {
            reject(new Error(`Failed to parse NMH JSON response: ${raw}`));
          }
        }
      }
    }

    function onErr(e) {
      cleanup();
      reject(e);
    }

    function cleanup() {
      clearTimeout(timer);
      proc.stdout.off("data", onData);
      proc.off("error", onErr);
    }

    proc.stdout.on("data", onData);
    proc.on("error", onErr);
  });
}

async function runTest() {
  console.log("[1/3] Testing edm-nmh get_diagnostics...");
  const pDiag = spawn(nmhPath, [`chrome-extension://${EXTENSION_ID}/`, "--parent-window=0"]);
  sendNativeMessage(pDiag, { type: "get_diagnostics", extension_id: EXTENSION_ID });
  const diagResp = await readNativeMessage(pDiag, 5000);
  pDiag.kill();

  assert.equal(diagResp.type, "diagnostics", "Response type must be 'diagnostics'");
  assert.ok(diagResp.report, "Diagnostics report must be present");
  assert.equal(diagResp.report.app_installed, true, "app_installed must be true");
  console.log("  ✓ NMH returned healthy diagnostics report");

  console.log("\n[2/3] Testing edm-nmh wake sequence...");
  const pWake = spawn(nmhPath, [`chrome-extension://${EXTENSION_ID}/`, "--parent-window=0"]);
  sendNativeMessage(pWake, { type: "wake", show_window: false });
  const wakeResp = await readNativeMessage(pWake, 15000);
  pWake.kill();

  assert.equal(wakeResp.type, "wake_result", "Response type must be 'wake_result'");
  assert.equal(wakeResp.ok, true, `Wake must succeed (error: ${wakeResp.error})`);
  assert.equal(wakeResp.status, "ready", "Status must be 'ready'");
  assert.ok(wakeResp.token && wakeResp.token.length >= 32, "Token must be valid");
  console.log(`  ✓ Wake succeeded in ~${wakeResp.time_to_ready_ms || "fast"}ms with token: ${wakeResp.token.slice(0, 8)}...`);

  console.log("\n[3/3] Testing loopback HTTP token endpoint (port 14370)...");
  const tokenFromHttp = await new Promise((resolve, reject) => {
    const req = http.request(
      "http://127.0.0.1:14370/token",
      {
        method: "GET",
        headers: {
          "X-EDM-Client": "extension",
          "X-EDM-Extension-Id": EXTENSION_ID,
        },
        timeout: 3000,
      },
      (res) => {
        let body = "";
        res.on("data", (c) => (body += c));
        res.on("end", () => {
          try {
            const data = JSON.parse(body);
            resolve(data.token);
          } catch (e) {
            reject(new Error(`Failed to parse HTTP body: ${body}`));
          }
        });
      }
    );
    req.on("error", reject);
    req.end();
  });

  assert.equal(tokenFromHttp, wakeResp.token, "HTTP endpoint token must match wake token");
  console.log("  ✓ HTTP Token verification on port 14370 matched authoritative token");

  console.log("\n=================================================");
  console.log(" [EDM Handoff Harness] All IPC & Wake Checks PASSED!");
  console.log("=================================================");
}

runTest().catch((err) => {
  console.error("\n✗ Handoff Harness Failed:", err.message);
  process.exit(1);
});

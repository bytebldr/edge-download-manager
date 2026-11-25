"use strict";
(() => {
  // src/utils/token.ts
  var DUMMY_TOKEN_PREFIX = "ed0000000000";
  var LOOPBACK_TOKEN_URLS = [
    "http://127.0.0.1:14370/token",
    "http://localhost:14370/token"
  ];
  function isValidToken(token) {
    return typeof token === "string" && token.trim().length >= 32 && !token.trim().startsWith(DUMMY_TOKEN_PREFIX);
  }
  async function autoDetectToken() {
    try {
      const nmhToken = await new Promise((resolve) => {
        chrome.runtime.sendNativeMessage("com.edm.nmh", { type: "get_token" }, (response) => {
          if (chrome.runtime.lastError || !response) {
            return resolve(null);
          }
          resolve(isValidToken(response.token) ? response.token.trim() : null);
        });
      });
      if (nmhToken) {
        await chrome.storage.local.set({ edm_ws_token: nmhToken });
        return nmhToken;
      }
    } catch {
    }
    const headers = {
      "X-EDM-Client": "extension"
    };
    if (typeof chrome !== "undefined" && chrome.runtime?.id) {
      headers["X-EDM-Extension-Id"] = chrome.runtime.id;
    }
    for (const url of LOOPBACK_TOKEN_URLS) {
      try {
        const resp = await fetch(url, {
          method: "GET",
          headers,
          cache: "no-store"
        });
        if (resp.ok) {
          const data = await resp.json();
          if (data && isValidToken(data.token)) {
            const token = data.token.trim();
            await chrome.storage.local.set({ edm_ws_token: token });
            return token;
          }
        }
      } catch {
      }
    }
    return null;
  }

  // src/options.ts
  var inputToken = document.getElementById("inputToken");
  var btnAutoDetect = document.getElementById("btnAutoDetect");
  var btnSave = document.getElementById("btnSave");
  var btnTest = document.getElementById("btnTest");
  var btnCheckUpdate = document.getElementById("btnCheckUpdate");
  var btnReloadExt = document.getElementById("btnReloadExt");
  var statusBadge = document.getElementById("statusBadge");
  var statusText = document.getElementById("statusText");
  var testFeedback = document.getElementById("testFeedback");
  var saveFeedback = document.getElementById("saveFeedback");
  var updateFeedback = document.getElementById("updateFeedback");
  var toggleInterception = document.getElementById("toggleInterception");
  var inputMinSize = document.getElementById("inputMinSize");
  var inputExcludedDomains = document.getElementById("inputExcludedDomains");
  var inputFileExtensions = document.getElementById("inputFileExtensions");
  var btnSaveFilters = document.getElementById("btnSaveFilters");
  var filterFeedback = document.getElementById("filterFeedback");
  var toggleAutoReconnect = document.getElementById("toggleAutoReconnect");
  var toggleNotifyComplete = document.getElementById("toggleNotifyComplete");
  var toggleNotifyFail = document.getElementById("toggleNotifyFail");
  var toggleYoutubePill = document.getElementById("toggleYoutubePill");
  async function loadSettings() {
    const stored = await chrome.storage.local.get([
      "edm_ws_token",
      "edm_interception_enabled",
      "edm_min_size",
      "edm_excluded_domains",
      "edm_file_extensions",
      "edm_auto_reconnect",
      "edm_notify_complete",
      "edm_notify_fail",
      "edm_youtube_pill_enabled"
    ]);
    if (isValidToken(stored.edm_ws_token)) {
      inputToken.value = stored.edm_ws_token.trim();
    } else {
      triggerAutoDetect(false);
    }
    toggleInterception.checked = stored.edm_interception_enabled !== false;
    if (inputMinSize) inputMinSize.value = stored.edm_min_size ? String(stored.edm_min_size) : "";
    if (inputExcludedDomains) inputExcludedDomains.value = stored.edm_excluded_domains || "";
    if (inputFileExtensions) inputFileExtensions.value = stored.edm_file_extensions || "";
    if (toggleAutoReconnect) toggleAutoReconnect.checked = stored.edm_auto_reconnect !== false;
    if (toggleNotifyComplete) toggleNotifyComplete.checked = stored.edm_notify_complete !== false;
    if (toggleNotifyFail) toggleNotifyFail.checked = stored.edm_notify_fail !== false;
    if (toggleYoutubePill) toggleYoutubePill.checked = stored.edm_youtube_pill_enabled !== false;
    checkStatus();
  }
  async function triggerAutoDetect(interactive = true) {
    if (interactive) {
      saveFeedback.className = "feedback-msg";
      saveFeedback.textContent = "Auto-detecting token from EDM Desktop app...";
      if (btnAutoDetect) btnAutoDetect.disabled = true;
    }
    const detected = await autoDetectToken();
    if (detected) {
      inputToken.value = detected;
      chrome.runtime.sendMessage({ action: "update_token", token: detected });
      if (btnAutoDetect) btnAutoDetect.disabled = false;
      saveFeedback.className = "feedback-msg success";
      saveFeedback.textContent = "Token auto-detected and synchronized successfully!";
      checkStatus();
      setTimeout(() => {
        saveFeedback.textContent = "";
      }, 3e3);
      return;
    }
    chrome.runtime.sendMessage({ action: "auto_detect_token" }, (resp) => {
      if (btnAutoDetect) btnAutoDetect.disabled = false;
      if (resp && resp.ok && resp.token) {
        inputToken.value = resp.token;
        saveFeedback.className = "feedback-msg success";
        saveFeedback.textContent = "Token auto-detected and synchronized successfully!";
        checkStatus();
        setTimeout(() => {
          saveFeedback.textContent = "";
        }, 3e3);
      } else if (interactive) {
        saveFeedback.className = "feedback-msg error";
        saveFeedback.textContent = resp?.error || "Could not auto-detect token. Make sure EDM Desktop is running.";
      }
    });
  }
  if (btnAutoDetect) {
    btnAutoDetect.addEventListener("click", () => triggerAutoDetect(true));
  }
  if (btnCheckUpdate) {
    btnCheckUpdate.addEventListener("click", () => {
      updateFeedback.className = "feedback-msg";
      updateFeedback.textContent = "Checking for extension updates...";
      btnCheckUpdate.disabled = true;
      chrome.runtime.sendMessage({ action: "check_update" }, (resp) => {
        btnCheckUpdate.disabled = false;
        if (resp && resp.ok) {
          if (resp.status === "update_available") {
            updateFeedback.className = "feedback-msg success";
            updateFeedback.textContent = `New update available (v${resp.version})! Click Reload Extension to apply.`;
          } else if (resp.status === "throttled") {
            updateFeedback.className = "feedback-msg";
            updateFeedback.textContent = "Update checks are throttled by Chrome. Try again shortly.";
          } else {
            updateFeedback.className = "feedback-msg success";
            updateFeedback.textContent = "Extension is fully up-to-date.";
          }
        } else {
          updateFeedback.className = "feedback-msg error";
          updateFeedback.textContent = resp?.error || "Unable to check for updates.";
        }
      });
    });
  }
  if (btnReloadExt) {
    btnReloadExt.addEventListener("click", () => {
      updateFeedback.className = "feedback-msg success";
      updateFeedback.textContent = "Reloading extension runtime...";
      chrome.runtime.sendMessage({ action: "reload_extension" });
      setTimeout(() => {
        window.location.reload();
      }, 300);
    });
  }
  function checkStatus() {
    chrome.runtime.sendMessage({ action: "get_status" }, (resp) => {
      if (chrome.runtime.lastError || !resp) {
        updateBadge("disconnected", "Desktop Unreachable");
        return;
      }
      if (resp.connectionStatus === "connected") {
        updateBadge("connected", `Connected (${resp.activeTasks || 0} active)`);
      } else if (resp.connectionStatus === "connecting" || resp.connectionStatus === "waking") {
        updateBadge("disconnected", "Connecting...");
      } else if (resp.connectionStatus === "asleep") {
        updateBadge("connected", "Asleep (Wakes on download)");
      } else {
        updateBadge("disconnected", "Disconnected");
      }
    });
  }
  function updateBadge(state, text) {
    statusBadge.className = `status-badge ${state}`;
    statusText.textContent = text;
  }
  btnSave.addEventListener("click", () => {
    const token = inputToken.value.trim();
    if (token.length < 32) {
      saveFeedback.className = "feedback-msg error";
      saveFeedback.textContent = "Token must be at least 32 characters long.";
      return;
    }
    chrome.runtime.sendMessage({ action: "update_token", token }, (resp) => {
      if (resp && resp.ok) {
        saveFeedback.className = "feedback-msg success";
        saveFeedback.textContent = "Token saved! Reconnecting...";
        setTimeout(() => {
          saveFeedback.textContent = "";
          checkStatus();
        }, 2e3);
      } else {
        saveFeedback.className = "feedback-msg error";
        saveFeedback.textContent = "Failed to update token.";
      }
    });
  });
  btnTest.addEventListener("click", async () => {
    let token = inputToken.value.trim();
    testFeedback.className = "feedback-msg";
    testFeedback.textContent = "Testing connection to 127.0.0.1:14370...";
    btnTest.disabled = true;
    if (!token || token.length < 32) {
      const stored = await chrome.storage.local.get("edm_ws_token");
      if (stored.edm_ws_token && stored.edm_ws_token.length >= 32) {
        token = stored.edm_ws_token;
        inputToken.value = token;
      }
    }
    chrome.runtime.sendMessage({ action: "test_token", token }, (resp) => {
      btnTest.disabled = false;
      if (chrome.runtime.lastError) {
        testFeedback.className = "feedback-msg error";
        testFeedback.textContent = "Background service worker not responding.";
        return;
      }
      if (resp && resp.ok) {
        testFeedback.className = "feedback-msg success";
        testFeedback.textContent = `Success! Connected in ${resp.latency}ms.`;
        updateBadge("connected", "Connected");
      } else {
        testFeedback.className = "feedback-msg error";
        testFeedback.textContent = `Connection failed: ${resp?.error || "Unknown error"}`;
        updateBadge("disconnected", "Connection Failed");
      }
    });
  });
  toggleInterception.addEventListener("change", () => {
    const enabled = toggleInterception.checked;
    chrome.storage.local.set({ edm_interception_enabled: enabled });
  });
  if (btnSaveFilters) {
    btnSaveFilters.addEventListener("click", async () => {
      const minSize = inputMinSize ? parseInt(inputMinSize.value, 10) || 0 : 0;
      const excludedDomains = inputExcludedDomains ? inputExcludedDomains.value.trim() : "";
      const fileExtensions = inputFileExtensions ? inputFileExtensions.value.trim() : "";
      await chrome.storage.local.set({
        edm_min_size: minSize,
        edm_excluded_domains: excludedDomains,
        edm_file_extensions: fileExtensions
      });
      if (filterFeedback) {
        filterFeedback.className = "feedback-msg success";
        filterFeedback.textContent = "Interception filters saved successfully!";
        setTimeout(() => {
          filterFeedback.textContent = "";
        }, 3e3);
      }
    });
  }
  if (toggleAutoReconnect) {
    toggleAutoReconnect.addEventListener("change", () => {
      chrome.storage.local.set({ edm_auto_reconnect: toggleAutoReconnect.checked });
    });
  }
  if (toggleNotifyComplete) {
    toggleNotifyComplete.addEventListener("change", () => {
      chrome.storage.local.set({ edm_notify_complete: toggleNotifyComplete.checked });
    });
  }
  if (toggleNotifyFail) {
    toggleNotifyFail.addEventListener("change", () => {
      chrome.storage.local.set({ edm_notify_fail: toggleNotifyFail.checked });
    });
  }
  if (toggleYoutubePill) {
    toggleYoutubePill.addEventListener("change", () => {
      chrome.storage.local.set({ edm_youtube_pill_enabled: toggleYoutubePill.checked });
    });
  }
  loadSettings();
  setInterval(checkStatus, 2500);
})();

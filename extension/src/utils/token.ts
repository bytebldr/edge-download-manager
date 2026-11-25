const DUMMY_TOKEN_PREFIX = "ed0000000000";
const LOOPBACK_TOKEN_URLS = [
  "http://127.0.0.1:14370/token",
  "http://localhost:14370/token",
];

export function isValidToken(token: unknown): token is string {
  return (
    typeof token === "string" &&
    token.trim().length >= 32 &&
    !token.trim().startsWith(DUMMY_TOKEN_PREFIX)
  );
}

/**
 * Auto-detects the EDM WebSocket authentication token from the Native Messaging Host
 * first, then falls back to the local HTTP loopback handshake endpoint.
 * Persists any discovered valid token to chrome.storage.local.
 */
export async function autoDetectToken(): Promise<string | null> {
  // 1. Try Native Messaging Host com.edm.nmh first (fastest, reads token directly)
  try {
    const nmhToken = await new Promise<string | null>((resolve) => {
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
  } catch {}

  // 2. Try HTTP loopback endpoints
  const headers: Record<string, string> = {
    "X-EDM-Client": "extension",
  };
  if (typeof chrome !== "undefined" && chrome.runtime?.id) {
    headers["X-EDM-Extension-Id"] = chrome.runtime.id;
  }

  for (const url of LOOPBACK_TOKEN_URLS) {
    try {
      const resp = await fetch(url, {
        method: "GET",
        headers,
        cache: "no-store",
      });
      if (resp.ok) {
        const data = await resp.json();
        if (data && isValidToken(data.token)) {
          const token = data.token.trim();
          await chrome.storage.local.set({ edm_ws_token: token });
          return token;
        }
      }
    } catch {}
  }

  return null;
}

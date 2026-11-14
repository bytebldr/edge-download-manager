/**
 * EDM Communication Contract - TypeScript Definitions
 * Canonical schema for Extension <-> Bridge <-> Desktop IPC.
 */

export const WS_PORT = 14370;
export const WS_URL = `ws://127.0.0.1:${WS_PORT}`;
export const HTTP_TOKEN_URL = `http://127.0.0.1:${WS_PORT}/token`;

export const CHROMIUM_EXTENSION_IDS = [
  "ginpdimhcdbojbhemgjedghkeolbcmgj",
  "ebdcbpghekpbgjiclaclbgmdnimeheci",
  "knldjmfmopnpolahpmmgbagdohdnhkik",
  "cphjfljhggknldkppchfobogknclmblg",
] as const;

export const FIREFOX_EXTENSION_ID = "edm@suvesh.dev";

export type ExtensionId = (typeof CHROMIUM_EXTENSION_IDS)[number] | typeof FIREFOX_EXTENSION_ID;

export interface InboundHelloMessage {
  type: "hello";
  token: string;
  browser?: string;
  version?: string;
}

export interface InboundCreateTaskMessage {
  type: "create_task";
  req_id: string;
  url: string;
  filename?: string;
  referrer?: string;
  user_agent?: string;
  cookies?: Array<{
    name: string;
    value: string;
    domain?: string;
    path?: string;
  }>;
}

export interface InboundActionMessage {
  type: "action";
  req_id: string;
  id: string;
  action: "pause" | "resume" | "cancel" | "remove" | "open_file" | "open_folder";
  delete_file?: boolean;
}

export interface InboundRefreshMessage {
  type: "refresh_task";
  req_id: string;
  task_id: string;
  new_url: string;
}

export interface OutboundTaskItem {
  id: string;
  filename: string;
  save_dir: string;
  total?: number;
  downloaded: number;
  speed: number;
  status: "queued" | "downloading" | "paused" | "completed" | "failed" | "cancelled";
  error_msg?: string;
}

export interface OutboundSnapshotMessage {
  type: "snapshot";
  epoch: string;
  rev: number;
  tasks: OutboundTaskItem[];
  theme?: string;
  palette?: string;
  lifecycle_state?: "active" | "warm" | "listener_sleep" | "deep_sleep";
}

export interface OutboundAckMessage {
  type: "ack";
  req_id: string;
  ok: boolean;
  id?: string;
  error?: string;
}

export interface NmhWakeRequest {
  type: "wake";
  show_window?: boolean;
}

export interface NmhWakeResponse {
  type: "wake_result";
  ok: boolean;
  status: "ready" | "error";
  token?: string;
  error?: string;
}

export interface NmhDiagnosticsReport {
  app_version: string;
  os: string;
  helper_path?: string;
  app_exe_path?: string;
  app_installed: boolean;
  ws_port_ready: boolean;
  time_to_ready_ms?: number | null;
  last_wake_error?: string | null;
  last_wake_timestamp?: string | null;
  browsers: Array<{
    browser: string;
    registry_key: string;
    helper_registered: boolean;
    manifest_path: string;
    manifest_exists: boolean;
    helper_path: string;
    helper_exists: boolean;
    origin_matched: boolean;
    error: string | null;
  }>;
  connected_extension_id?: string | null;
}

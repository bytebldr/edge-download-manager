import React, { useCallback, useEffect, useState } from "react";
import { Tooltip } from "../../components/Tooltip";
import {
  AlertCircle,
  CheckCircle2,
  Cpu,
  Eye,
  EyeOff,
  RefreshCw,
  Save,
  Send,
  Terminal,
  Webhook,
} from "lucide-react";

export interface AutomationConfig {
  rpcEnabled: boolean;
  rpcPort: number;
  rpcSecret: string;
  webhookUrl: string | null;
  onCompleteEnabled: boolean;
  trimMemoryOnTray: boolean;
}

const DEFAULT_CONFIG: AutomationConfig = {
  rpcEnabled: false,
  rpcPort: 6800,
  rpcSecret: "",
  webhookUrl: null,
  onCompleteEnabled: false,
  trimMemoryOnTray: true,
};

const STORAGE_KEY = "edm_automation_config";

const isTauriRuntime = (): boolean =>
  typeof window !== "undefined" && ("__TAURI_INTERNALS__" in window || "__TAURI__" in window);

async function invokeAutomation<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauriRuntime()) {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<T>(cmd, args);
  }

  if (cmd === "get_automation_config") {
    const raw = localStorage.getItem(STORAGE_KEY);
    return (raw ? { ...DEFAULT_CONFIG, ...JSON.parse(raw) } : DEFAULT_CONFIG) as unknown as T;
  }
  if (cmd === "save_automation_config") {
    const cfg = args?.config as AutomationConfig;
    localStorage.setItem(STORAGE_KEY, JSON.stringify(cfg));
    return cfg as unknown as T;
  }
  if (cmd === "test_automation_webhook") {
    const url = String(args?.url ?? "").trim();
    if (!/^https?:\/\//i.test(url)) {
      throw new Error("Webhook URL must use http:// or https://");
    }
    return 200 as unknown as T;
  }
  if (cmd === "trim_idle_webview_memory") {
    return true as unknown as T;
  }
  throw new Error(`Unsupported command: ${cmd}`);
}

function generateSecret(): string {
  const bytes = new Uint8Array(16);
  if (typeof crypto !== "undefined" && "getRandomValues" in crypto) {
    crypto.getRandomValues(bytes);
  } else {
    for (let i = 0; i < bytes.length; i++) {
      bytes[i] = Math.floor(Math.random() * 256);
    }
  }
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

export const AutomationPanel: React.FC = () => {
  const [config, setConfig] = useState<AutomationConfig>(DEFAULT_CONFIG);
  const [showSecret, setShowSecret] = useState(false);
  const [saving, setSaving] = useState(false);
  const [testingWebhook, setTestingWebhook] = useState(false);
  const [trimming, setTrimming] = useState(false);
  const [statusMsg, setStatusMsg] = useState<{ kind: "ok" | "err"; text: string } | null>(null);

  const loadConfig = useCallback(async () => {
    try {
      const loaded = await invokeAutomation<AutomationConfig>("get_automation_config");
      setConfig({ ...DEFAULT_CONFIG, ...loaded });
    } catch (err) {
      setStatusMsg({
        kind: "err",
        text: err instanceof Error ? err.message : String(err),
      });
    }
  }, []);

  useEffect(() => {
    void loadConfig();
  }, [loadConfig]);

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();
    setSaving(true);
    setStatusMsg(null);
    try {
      const saved = await invokeAutomation<AutomationConfig>("save_automation_config", {
        config: {
          ...config,
          webhookUrl: config.webhookUrl?.trim() ? config.webhookUrl.trim() : null,
        },
      });
      setConfig(saved);
      setStatusMsg({ kind: "ok", text: "Automation settings saved." });
    } catch (err) {
      setStatusMsg({
        kind: "err",
        text: err instanceof Error ? err.message : String(err),
      });
    } finally {
      setSaving(false);
    }
  };

  const handleTestWebhook = async () => {
    const url = config.webhookUrl?.trim() ?? "";
    if (!url) {
      setStatusMsg({ kind: "err", text: "Enter a webhook URL first." });
      return;
    }
    setTestingWebhook(true);
    setStatusMsg(null);
    try {
      const code = await invokeAutomation<number>("test_automation_webhook", { url });
      setStatusMsg({ kind: "ok", text: `Webhook test succeeded (HTTP ${code}).` });
    } catch (err) {
      setStatusMsg({
        kind: "err",
        text: err instanceof Error ? err.message : String(err),
      });
    } finally {
      setTestingWebhook(false);
    }
  };

  const handleTrimNow = async () => {
    setTrimming(true);
    setStatusMsg(null);
    try {
      const trimmed = await invokeAutomation<boolean>("trim_idle_webview_memory");
      setStatusMsg({
        kind: "ok",
        text: trimmed
          ? "Working set memory trimmed."
          : "Memory trim is only active on Windows.",
      });
    } catch (err) {
      setStatusMsg({
        kind: "err",
        text: err instanceof Error ? err.message : String(err),
      });
    } finally {
      setTrimming(false);
    }
  };

  return (
    <form
      onSubmit={handleSave}
      className="flex flex-col gap-4 p-4 rounded-2xl bg-[var(--bg-panel)] border border-[var(--border-subtle)] text-[var(--text-primary)]"
    >
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-2.5">
          <Terminal className="w-5 h-5 text-[var(--accent)]" />
          <div>
            <h2 className="text-sm font-semibold tracking-tight">Automation &amp; RPC</h2>
            <p className="text-xs text-[var(--text-muted)]">
              Configure aria2 JSON-RPC control, completion webhooks, and idle tray memory trim.
            </p>
          </div>
        </div>
        <button
          type="submit"
          disabled={saving}
          className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium bg-[var(--accent)] text-[var(--accent-on)] hover:bg-[var(--accent-hover)] disabled:opacity-50 transition-colors cursor-pointer"
        >
          <Save className="w-3.5 h-3.5" />
          {saving ? "Saving..." : "Save"}
        </button>
      </div>

      {statusMsg && (
        <div
          className={`flex items-center gap-2 px-3 py-2 rounded-lg text-xs border ${
            statusMsg.kind === "ok"
              ? "border-[var(--accent)]/40 bg-[var(--accent)]/10 text-[var(--text-primary)]"
              : "border-[var(--danger)]/40 bg-[var(--danger)]/10 text-[var(--danger)]"
          }`}
        >
          {statusMsg.kind === "ok" ? (
            <CheckCircle2 className="w-4 h-4 text-[var(--accent)] shrink-0" />
          ) : (
            <AlertCircle className="w-4 h-4 text-[var(--danger)] shrink-0" />
          )}
          <span>{statusMsg.text}</span>
        </div>
      )}

      {/* aria2 JSON-RPC */}
      <div className="flex flex-col gap-3 p-3.5 rounded-xl bg-[var(--bg-card)] border border-[var(--border-subtle)]">
        <div className="flex items-center justify-between gap-3">
          <div className="flex items-center gap-2">
            <Terminal className="w-4 h-4 text-[var(--accent)]" />
            <span className="text-xs font-semibold">aria2 JSON-RPC 2.0</span>
          </div>
          <label className="inline-flex items-center gap-2 text-xs cursor-pointer select-none">
            <input
              type="checkbox"
              checked={config.rpcEnabled}
              onChange={(e) => setConfig({ ...config, rpcEnabled: e.target.checked })}
              className="accent-[var(--accent)]"
            />
            <span>{config.rpcEnabled ? "Enabled" : "Disabled"}</span>
          </label>
        </div>

        <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
          <label className="flex flex-col gap-1 text-xs">
            <span className="text-[var(--text-secondary)]">RPC Port</span>
            <input
              type="number"
              min={1024}
              max={65535}
              value={config.rpcPort}
              onChange={(e) =>
                setConfig({
                  ...config,
                  rpcPort: Math.min(65535, Math.max(1024, Number(e.target.value) || 6800)),
                })
              }
              className="px-2.5 py-1.5 rounded-lg bg-[var(--bg-panel)] border border-[var(--border-subtle)] text-[var(--text-primary)] focus:outline-none focus:border-[var(--accent)]"
            />
          </label>

          <div className="sm:col-span-2 flex flex-col gap-1 text-xs">
            <span className="text-[var(--text-secondary)]">RPC Secret Token</span>
            <div className="flex items-center gap-1.5">
              <input
                type={showSecret ? "text" : "password"}
                value={config.rpcSecret}
                onChange={(e) => setConfig({ ...config, rpcSecret: e.target.value })}
                placeholder="Optional token:<secret>"
                className="flex-1 px-2.5 py-1.5 rounded-lg bg-[var(--bg-panel)] border border-[var(--border-subtle)] text-[var(--text-primary)] focus:outline-none focus:border-[var(--accent)]"
              />
              <button
                type="button"
                onClick={() => setShowSecret((v) => !v)}
                className="p-1.5 rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-panel)] text-[var(--text-muted)] hover:text-[var(--text-primary)] cursor-pointer"
                aria-label={showSecret ? "Hide secret" : "Show secret"}
              >
                {showSecret ? <EyeOff className="w-3.5 h-3.5" /> : <Eye className="w-3.5 h-3.5" />}
              </button>
              <Tooltip content="Generate random secret" side="top" align="end">
                <button
                  type="button"
                  onClick={() => setConfig({ ...config, rpcSecret: generateSecret() })}
                  aria-label="Generate random secret"
                  className="inline-flex items-center gap-1 px-2.5 py-1.5 rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-panel)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] cursor-pointer"
                >
                  <RefreshCw className="w-3.5 h-3.5" />
                  <span>Generate</span>
                </button>
              </Tooltip>
            </div>
          </div>
        </div>
      </div>

      {/* Completion Webhook */}
      <div className="flex flex-col gap-3 p-3.5 rounded-xl bg-[var(--bg-card)] border border-[var(--border-subtle)]">
        <div className="flex items-center justify-between gap-3">
          <div className="flex items-center gap-2">
            <Webhook className="w-4 h-4 text-[var(--accent)]" />
            <span className="text-xs font-semibold">Completion Webhook</span>
          </div>
          <label className="inline-flex items-center gap-2 text-xs cursor-pointer select-none">
            <input
              type="checkbox"
              checked={config.onCompleteEnabled}
              onChange={(e) => setConfig({ ...config, onCompleteEnabled: e.target.checked })}
              className="accent-[var(--accent)]"
            />
            <span>Notify on complete</span>
          </label>
        </div>

        <div className="flex items-center gap-2">
          <input
            type="url"
            value={config.webhookUrl ?? ""}
            onChange={(e) => setConfig({ ...config, webhookUrl: e.target.value || null })}
            placeholder="https://hooks.example.com/edm-complete"
            className="flex-1 px-2.5 py-1.5 rounded-lg text-xs bg-[var(--bg-panel)] border border-[var(--border-subtle)] text-[var(--text-primary)] focus:outline-none focus:border-[var(--accent)]"
          />
          <button
            type="button"
            disabled={testingWebhook}
            onClick={handleTestWebhook}
            className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium border border-[var(--border-subtle)] bg-[var(--bg-panel)] text-[var(--text-primary)] hover:bg-[var(--bg-card-hover)] disabled:opacity-50 cursor-pointer"
          >
            <Send className="w-3.5 h-3.5" />
            {testingWebhook ? "Testing..." : "Test Webhook"}
          </button>
        </div>
      </div>

      {/* Memory Trim */}
      <div className="flex items-center justify-between gap-3 p-3.5 rounded-xl bg-[var(--bg-card)] border border-[var(--border-subtle)]">
        <div className="flex items-center gap-2.5">
          <Cpu className="w-4 h-4 text-[var(--accent)]" />
          <div>
            <div className="text-xs font-semibold">Tray Idle Memory Trim</div>
            <div className="text-[11px] text-[var(--text-muted)]">
              Trim working set memory when minimized to the system tray.
            </div>
          </div>
        </div>
        <div className="flex items-center gap-3">
          <button
            type="button"
            disabled={trimming}
            onClick={handleTrimNow}
            className="px-2.5 py-1 rounded-lg text-xs border border-[var(--border-subtle)] bg-[var(--bg-panel)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] disabled:opacity-50 cursor-pointer"
          >
            {trimming ? "Trimming..." : "Trim Now"}
          </button>
          <label className="inline-flex items-center gap-2 text-xs cursor-pointer select-none">
            <input
              type="checkbox"
              checked={config.trimMemoryOnTray}
              onChange={(e) => setConfig({ ...config, trimMemoryOnTray: e.target.checked })}
              className="accent-[var(--accent)]"
            />
            <span>{config.trimMemoryOnTray ? "On" : "Off"}</span>
          </label>
        </div>
      </div>
    </form>
  );
};

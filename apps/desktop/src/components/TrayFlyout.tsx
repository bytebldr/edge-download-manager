import { useCallback, useEffect, useState } from "react";
import { ArrowDownToLine, ExternalLink, Pause, Play, X } from "lucide-react";
import {
  BackendDownloadItem,
  isTauri,
  tauriCancelDownload,
  tauriListDownloads,
  tauriPauseDownload,
  tauriResumeDownload,
} from "../services/tauriBridge";

const ACTIVE_STATES = new Set(["downloading", "probing", "queued", "verifying", "paused"]);

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const idx = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const val = bytes / Math.pow(1024, idx);
  return `${val >= 100 || idx === 0 ? val.toFixed(0) : val.toFixed(1)} ${units[idx]}`;
}

function formatEta(seconds?: number | null): string {
  if (seconds == null || !Number.isFinite(seconds) || seconds < 0) return "--";
  if (seconds < 60) return `${Math.ceil(seconds)}s`;
  const mins = Math.floor(seconds / 60);
  const secs = Math.floor(seconds % 60);
  if (mins < 60) return `${mins}m ${secs}s`;
  const hrs = Math.floor(mins / 60);
  return `${hrs}h ${mins % 60}m`;
}

export default function TrayFlyout() {
  const [items, setItems] = useState<BackendDownloadItem[]>([]);
  const [busyId, setBusyId] = useState<string | null>(null);

  const refreshDownloads = useCallback(async () => {
    const list = await tauriListDownloads();
    if (!list) return;
    setItems(list.filter((d) => ACTIVE_STATES.has(d.state)));
  }, []);

  useEffect(() => {
    void refreshDownloads();
    const timer = window.setInterval(() => {
      void refreshDownloads();
    }, 800);
    return () => window.clearInterval(timer);
  }, [refreshDownloads]);

  const hideFlyout = useCallback(async () => {
    if (!isTauri()) return;
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      await getCurrentWindow().hide();
    } catch (err) {
      console.warn("Failed to hide tray flyout:", err);
    }
  }, []);

  const handleOpenMain = useCallback(async () => {
    if (isTauri()) {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        await invoke("wake_engine", { showWindow: true, show_window: true });
      } catch (err) {
        console.warn("Failed to wake main window:", err);
      }
    }
    await hideFlyout();
  }, [hideFlyout]);

  const handleTogglePause = async (item: BackendDownloadItem) => {
    setBusyId(item.id);
    try {
      if (item.state === "paused") {
        await tauriResumeDownload(item.id);
      } else {
        await tauriPauseDownload(item.id);
      }
      await refreshDownloads();
    } finally {
      setBusyId(null);
    }
  };

  const handleCancel = async (id: string) => {
    setBusyId(id);
    try {
      await tauriCancelDownload(id);
      await refreshDownloads();
    } finally {
      setBusyId(null);
    }
  };

  return (
    <div className="h-screen w-screen bg-transparent p-1.5 select-none">
      <div
        className="flex h-full w-full flex-col overflow-hidden rounded-xl border border-[var(--border-subtle)] bg-[#0A0A0A]/95 text-[var(--text-primary)] shadow-[var(--shadow-depth)] backdrop-blur-xl"
        style={{ boxShadow: "var(--shadow-inset-bevel), var(--shadow-depth)" }}
      >
        {/* Header */}
        <div className="flex items-center justify-between border-b border-[var(--border-dim)] px-3.5 py-2.5">
          <div className="flex items-center gap-2">
            <span className="h-2 w-2 rounded-full bg-[var(--accent)]" />
            <h1 className="text-[13px] font-semibold tracking-tight text-[var(--text-primary)]">
              Downloads in progress
            </h1>
            {items.length > 0 && (
              <span className="rounded-full bg-[var(--accent-badge-bg)] px-1.5 py-0.5 text-[10px] font-medium text-[var(--accent-badge-text)]">
                {items.length}
              </span>
            )}
          </div>
          <button
            type="button"
            onClick={() => void hideFlyout()}
            aria-label="Close tray flyout"
            className="rounded-md p-1 text-[var(--text-muted)] transition-colors hover:bg-white/10 hover:text-[var(--text-primary)]"
          >
            <X className="h-3.5 w-3.5" />
          </button>
        </div>

        {/* Active downloads list (up to 3 visible, scrollable if >3) */}
        <div className="flex-1 overflow-y-auto px-3 py-2 space-y-2 max-h-[210px]">
          {items.length === 0 ? (
            <div className="flex h-full min-h-[120px] flex-col items-center justify-center text-center px-4">
              <div className="mb-2 flex h-8 w-8 items-center justify-center rounded-full bg-white/5 text-[var(--text-muted)]">
                <ArrowDownToLine className="h-4 w-4" />
              </div>
              <p className="text-xs font-medium text-[var(--text-secondary)]">No active downloads</p>
              <p className="mt-0.5 text-[11px] text-[var(--text-muted)]">
                New downloads will appear here
              </p>
            </div>
          ) : (
            items.map((item) => {
              const total = item.size_total ?? 0;
              const pct = total > 0 ? Math.min(100, Math.round((item.size_done / total) * 100)) : 0;
              const isPaused = item.state === "paused";
              const speed = item.speed_bytes_per_sec > 0 ? `${formatBytes(item.speed_bytes_per_sec)}/s` : "0 B/s";
              const meta = isPaused
                ? `${formatBytes(item.size_done)} / ${total > 0 ? formatBytes(total) : "?"} · Paused`
                : `${formatBytes(item.size_done)} / ${total > 0 ? formatBytes(total) : "?"} · ${speed} · ${formatEta(item.eta_seconds)}`;

              return (
                <div
                  key={item.id}
                  className="rounded-lg border border-[var(--border-dim)] bg-[var(--bg-card)] px-2.5 py-2 transition-colors hover:border-[var(--border-subtle)]"
                >
                  <div className="flex items-center justify-between gap-2">
                    <span className="truncate text-xs font-medium text-[var(--text-primary)]" title={item.filename}>
                      {item.filename}
                    </span>
                    <div className="flex shrink-0 items-center gap-1">
                      <button
                        type="button"
                        disabled={busyId === item.id}
                        onClick={() => void handleTogglePause(item)}
                        title={isPaused ? "Resume" : "Pause"}
                        className="rounded p-1 text-[var(--text-secondary)] transition-colors hover:bg-white/10 hover:text-[var(--text-primary)] disabled:opacity-40"
                      >
                        {isPaused ? <Play className="h-3 w-3" /> : <Pause className="h-3 w-3" />}
                      </button>
                      <button
                        type="button"
                        disabled={busyId === item.id}
                        onClick={() => void handleCancel(item.id)}
                        title="Cancel"
                        className="rounded p-1 text-[var(--text-secondary)] transition-colors hover:bg-red-500/15 hover:text-[var(--danger)] disabled:opacity-40"
                      >
                        <X className="h-3 w-3" />
                      </button>
                    </div>
                  </div>
                  <div className="mt-1.5 h-1.5 w-full overflow-hidden rounded-full bg-white/10">
                    <div
                      className={`h-full rounded-full transition-all duration-300 ${
                        isPaused ? "bg-[var(--warning)]" : "bg-[var(--accent)]"
                      }`}
                      style={{ width: `${pct}%` }}
                    />
                  </div>
                  <div className="mt-1 flex items-center justify-between text-[10px] text-[var(--text-muted)] tabular-nums">
                    <span className="truncate">{meta}</span>
                    <span className="ml-2 shrink-0 font-medium text-[var(--text-secondary)]">{pct}%</span>
                  </div>
                </div>
              );
            })
          )}
        </div>

        {/* Footer */}
        <div className="border-t border-[var(--border-dim)] p-2.5">
          <button
            type="button"
            onClick={() => void handleOpenMain()}
            className="flex w-full items-center justify-center gap-1.5 rounded-lg bg-[var(--accent)] px-3 py-1.5 text-xs font-medium text-[var(--accent-on)] transition-colors hover:bg-[var(--accent-hover)] active:bg-[var(--accent-pressed)]"
          >
            <ExternalLink className="h-3.5 w-3.5" />
            Open Evo Download Manager
          </button>
        </div>
      </div>
    </div>
  );
}

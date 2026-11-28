import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ArrowDownToLine, ExternalLink, Pause, Play, X } from "lucide-react";
import { Tooltip } from "./Tooltip";
import {
  BackendDownloadItem,
  isTauri,
  tauriCancelDownload,
  tauriListDownloads,
  tauriLoadSettings,
  tauriPauseDownload,
  tauriResumeDownload,
  tauriWindowClose,
} from "../services/tauriBridge";
import { loadSettings } from "../types/settings";
import { applyFontFamily } from "../lib/fonts";

const ACTIVE_STATES = new Set(["downloading", "probing", "queued", "verifying", "paused"]);

function stateRank(state: string): number {
  if (state === "paused") return 0;
  if (state === "queued") return 1;
  return 2;
}

function createdAtMs(createdAt: string): number {
  const ms = Date.parse(createdAt);
  return Number.isFinite(ms) ? ms : 0;
}

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
  const listRef = useRef<HTMLDivElement>(null);
  const autoScrollRef = useRef(true);

  const sortedItems = useMemo(() => {
    return [...items].sort((a, b) => {
      const rankDiff = stateRank(a.state) - stateRank(b.state);
      if (rankDiff !== 0) return rankDiff;
      if (a.state === "queued" && b.state === "queued") {
        // Newest at top, oldest (#1 next-to-run) directly above active downloads
        return createdAtMs(b.created_at) - createdAtMs(a.created_at);
      }
      return createdAtMs(a.created_at) - createdAtMs(b.created_at);
    });
  }, [items]);

  const queueOrder = useMemo(() => {
    const map = new Map<string, number>();
    let n = 1;
    for (let i = sortedItems.length - 1; i >= 0; i--) {
      const item = sortedItems[i];
      if (item.state === "queued") {
        map.set(item.id, n++);
      }
    }
    return map;
  }, [sortedItems]);

  useEffect(() => {
    const el = listRef.current;
    if (el && autoScrollRef.current) {
      el.scrollTop = el.scrollHeight;
    }
  }, [sortedItems]);

  const refreshDownloads = useCallback(async () => {
    if (!isTauri()) {
      return;
    }
    const list = await tauriListDownloads();
    if (!list) return;
    setItems(list.filter((d) => ACTIVE_STATES.has(d.state)));
  }, []);

  useEffect(() => {
    const syncAppearance = async () => {
      const local = loadSettings();
      const remote = isTauri() ? await tauriLoadSettings().catch(() => null) : null;
      const g = remote?.general ?? local.general;
      const resolvedTheme =
        g.theme === "system"
          ? window.matchMedia("(prefers-color-scheme: light)").matches
            ? "light"
            : "dark"
          : g.theme;
      document.documentElement.setAttribute("data-theme", resolvedTheme);
      document.documentElement.setAttribute("data-palette", g.palette || "blue");
      applyFontFamily(g.fontFamily);
    };
    void syncAppearance();
    const onSync = () => void syncAppearance();
    window.addEventListener("focus", onSync);
    window.addEventListener("storage", onSync);
    return () => {
      window.removeEventListener("focus", onSync);
      window.removeEventListener("storage", onSync);
    };
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
      await tauriWindowClose();
    } catch (err) {
      console.warn("Failed to close tray flyout:", err);
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
      if (!isTauri()) {
        setItems((prev) =>
          prev.map((d) =>
            d.id === item.id
              ? {
                  ...d,
                  state: item.state === "paused" ? "downloading" : "paused",
                  speed_bytes_per_sec: item.state === "paused" ? 8_800_000 : 0,
                }
              : d
          )
        );
        return;
      }
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
      if (!isTauri()) {
        setItems((prev) => prev.filter((d) => d.id !== id));
        return;
      }
      await tauriCancelDownload(id);
      await refreshDownloads();
    } finally {
      setBusyId(null);
    }
  };

  const totalSpeedBps = useMemo(
    () => items.reduce((sum, d) => sum + (d.state === "downloading" ? d.speed_bytes_per_sec || 0 : 0), 0),
    [items]
  );

  const totalUploadBps = useMemo(
    () => items.reduce((sum, d) => sum + (d.state === "downloading" ? (d as any).upload_bytes_per_sec || 0 : 0), 0),
    [items]
  );

  return (
    <div className="h-screen w-screen bg-transparent p-1.5 select-none">
      <div className="flex h-full w-full flex-col overflow-hidden rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-chassis)] text-[var(--text-primary)] shadow-[0_24px_48px_-12px_rgba(0,0,0,0.75)]">
        {/* Header */}
        <div className="header-3d flex items-center justify-between px-3.5 py-2.5 shrink-0">
          <div className="flex items-center gap-2 min-w-0">
            <img src="/logo.png" alt="EDM" className="w-4 h-4 object-contain shrink-0 pointer-events-none" />
            <h1 className="text-xs font-semibold tracking-tight text-[var(--text-primary)] truncate">
              EDM
            </h1>
            {items.length > 0 && (
              <span className="text-[11px] font-medium tabular-nums text-[var(--text-muted)]">
                ({items.length})
              </span>
            )}
          </div>
          <Tooltip content="Hide" side="bottom" align="end">
            <button
              type="button"
              onClick={() => void hideFlyout()}
              aria-label="Close tray flyout"
              className="w-7 h-7 rounded-lg flex items-center justify-center text-[var(--text-muted)] transition-colors hover:bg-white/[0.06] hover:text-[var(--text-primary)] cursor-pointer"
            >
              <X className="w-4 h-4 stroke-[1.75]" />
            </button>
          </Tooltip>
        </div>

        {/* Active downloads list (Edge-to-edge hairline rows) */}
        <div
          ref={listRef}
          onScroll={(e) => {
            const el = e.currentTarget;
            autoScrollRef.current = el.scrollHeight - el.scrollTop - el.clientHeight < 24;
          }}
          className="flex-1 overflow-y-auto divide-y divide-[var(--border-dim)]"
        >
          {sortedItems.length === 0 ? (
            <div className="flex h-full min-h-[140px] flex-col items-center justify-center text-center px-4">
              <div className="mb-2.5 flex h-9 w-9 items-center justify-center rounded-lg bg-white/[0.04] border border-[var(--border-dim)] text-[var(--text-muted)]">
                <ArrowDownToLine className="h-4 w-4 stroke-[1.75]" />
              </div>
              <p className="text-xs font-medium text-[var(--text-primary)]">No active downloads</p>
              <p className="mt-0.5 text-[11px] text-[var(--text-muted)]">
                New transfers will appear here
              </p>
            </div>
          ) : (
            sortedItems.map((item) => {
              const total = item.size_total ?? 0;
              const pct = total > 0 ? Math.min(100, Math.round((item.size_done / total) * 100)) : 0;
              const isPaused = item.state === "paused";
              const isDownloading = item.state === "downloading";
              const queuePos = queueOrder.get(item.id);
              const isQueued = queuePos != null || item.state === "queued";
              const speed = item.speed_bytes_per_sec > 0 ? `${formatBytes(item.speed_bytes_per_sec)}/s` : "0 B/s";
              const meta = isPaused
                ? `${formatBytes(item.size_done)} / ${total > 0 ? formatBytes(total) : "?"} • Paused`
                : isQueued
                  ? `${total > 0 ? formatBytes(total) : "?"} • Queued${queuePos ? ` #${queuePos}` : ""}`
                  : `${formatBytes(item.size_done)} / ${total > 0 ? formatBytes(total) : "?"} • ${speed} • ${formatEta(item.eta_seconds)}`;

              return (
                <div
                  key={item.id}
                  className="px-3.5 py-2.5 transition-colors hover:bg-white/[0.025]"
                >
                  <div className="flex items-center justify-between gap-2">
                    <div className="flex min-w-0 items-center gap-2">
                      <span
                        className={`w-1.5 h-1.5 rounded-full shrink-0 ${
                          isDownloading
                            ? "bg-[var(--accent)]"
                            : isPaused
                              ? "bg-amber-400"
                              : "bg-zinc-500"
                        }`}
                      />
                      <span className="truncate text-xs font-medium text-[var(--text-primary)]" title={item.filename}>
                        {item.filename}
                      </span>
                    </div>
                    <div className="inline-flex shrink-0 items-center p-0.5 rounded-full bg-white/[0.03] border border-[var(--border-dim)]">
                      <Tooltip content={isPaused ? "Resume" : isQueued ? "Start" : "Pause"} side="top">
                        <button
                          type="button"
                          disabled={busyId === item.id}
                          onClick={() => void handleTogglePause(item)}
                          aria-label={isPaused ? "Resume" : "Pause"}
                          className={`w-6 h-6 rounded-full flex items-center justify-center transition-colors disabled:opacity-40 cursor-pointer ${
                            isPaused
                              ? "text-amber-400 hover:bg-amber-500/15"
                              : isDownloading
                                ? "text-[var(--accent)] hover:bg-[var(--accent)]/15"
                                : "text-[var(--text-secondary)] hover:bg-white/[0.08] hover:text-[var(--text-primary)]"
                          }`}
                        >
                          {isPaused || isQueued ? (
                            <Play className="w-3 h-3 stroke-[1.75]" />
                          ) : (
                            <Pause className="w-3 h-3 stroke-[1.75]" />
                          )}
                        </button>
                      </Tooltip>
                      <Tooltip content="Cancel" side="top" align="end">
                        <button
                          type="button"
                          disabled={busyId === item.id}
                          onClick={() => void handleCancel(item.id)}
                          aria-label="Cancel"
                          className="w-6 h-6 rounded-full flex items-center justify-center text-[var(--text-secondary)] transition-colors hover:bg-rose-500/15 hover:text-rose-400 disabled:opacity-40 cursor-pointer"
                        >
                          <X className="w-3 h-3 stroke-[1.75]" />
                        </button>
                      </Tooltip>
                    </div>
                  </div>
                  <div className="mt-2 h-1.5 w-full overflow-hidden rounded-full bg-white/[0.06] relative">
                    {isQueued && pct === 0 ? (
                      <div className="w-1/4 h-full rounded-full bg-gradient-to-r from-transparent via-zinc-400/70 to-transparent edm-queue-sweep" />
                    ) : (
                      <>
                        <div
                          className={`h-full rounded-full transition-[width,background-color] duration-200 ${
                            isPaused ? "bg-amber-500/85" : "bg-[var(--accent)]"
                          }`}
                          style={{ width: `${pct}%` }}
                        />
                        {(isDownloading || isPaused) && pct < 100 && (
                          <div
                            className="absolute inset-y-0 right-0 overflow-hidden pointer-events-none"
                            style={{ left: `${pct}%` }}
                          >
                            <div
                              className={`w-1/3 h-full rounded-full bg-gradient-to-r from-transparent ${
                                isPaused ? "via-amber-300/65" : "via-[var(--accent)]/60"
                              } to-transparent edm-queue-sweep`}
                            />
                          </div>
                        )}
                      </>
                    )}
                  </div>
                  <div className="mt-1.5 flex items-center justify-between text-[11px] text-[var(--text-muted)] tabular-nums">
                    <span className="truncate">{meta}</span>
                    <span className="ml-2 shrink-0 font-medium text-[var(--text-secondary)]">{pct}%</span>
                  </div>
                </div>
              );
            })
          )}
        </div>

        {/* Footer */}
        <div className="footer-3d flex items-center justify-between px-3.5 py-2.5 shrink-0">
          <div className="flex items-center gap-3 text-[11px] font-medium tabular-nums text-[var(--text-secondary)]">
            <span className={totalSpeedBps > 0 ? "text-[var(--accent)]" : "text-[var(--text-muted)]"}>
              ↓ {totalSpeedBps > 0 ? `${formatBytes(totalSpeedBps)}/s` : "0 B/s"}
            </span>
            <span className={totalUploadBps > 0 ? "text-[#818CF8]" : "text-[var(--text-muted)]"}>
              ↑ {totalUploadBps > 0 ? `${formatBytes(totalUploadBps)}/s` : "0 B/s"}
            </span>
          </div>
          <button
            type="button"
            onClick={() => void handleOpenMain()}
            className="h-7 px-3 rounded-lg bg-white/[0.06] border border-[var(--border-subtle)] text-xs font-medium text-[var(--text-primary)] hover:bg-white/[0.1] transition-colors inline-flex items-center gap-1.5 cursor-pointer"
          >
            <ExternalLink className="w-3.5 h-3.5 stroke-[1.75]" />
            <span>Open EDM</span>
          </button>
        </div>
      </div>
    </div>
  );
}

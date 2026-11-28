import React, { useState, useEffect, useRef, useMemo } from "react";
import {
  Download,
  Pause,
  Play,
  Trash2,
  FolderOpen,
  Plus,
  FileCode,
  Film,
  Music,
  Archive,
  FileText,
  X,
  Layers,
  Clock,
  CheckCircle2,
  LayoutList,
  ChevronDown,
  ChevronUp,
  Cpu,
  RotateCcw,
  Image,
  CalendarClock,
  Zap,
  Settings,
  Info,
} from "lucide-react";
import SettingsPage from "./components/SettingsPage";
import SetupWizard from "./components/SetupWizard";
import { RefreshLinkModal } from "./components/RefreshLinkModal";
import { PowerCountdownModal } from "./components/PowerCountdownModal";
import { AddDownloadModal } from "./components/AddDownloadModal";
import { DetailsInspectorDrawer } from "./components/DetailsInspectorDrawer";
import { AppToolbar } from "./components/AppToolbar";
import { StatusFooter } from "./components/StatusFooter";
import { Tooltip } from "./components/Tooltip";
import { DownloadContextMenu } from "./components/DownloadContextMenu";
import {
  APP_VERSION,
  DOWNLOADS_STORAGE_KEY,
  DISMISSED_UPDATE_VERSION_KEY,
  DEFAULT_MAX_CONNECTIONS,
  DEFAULT_FALLBACK_SIZE_BYTES,
  BACKEND_SYNC_INTERVAL_MS,
  TELEMETRY_TICK_INTERVAL_MS,
  UI_THROTTLE_MS,
  STALLED_THRESHOLD_MS,
  LOCAL_PREVIEW_GRACE_MS,
  SPEED_LIMIT_BYTES_MAP,
  VALID_DOWNLOAD_CATEGORIES,
  BACKEND_STATE_MAP,
  inferCategoryFromExtension,
  ensureSingleEdmPath,
  buildCategoryFolderMap,
} from "./config/constants";
import { BatchImportModal, ImportedLink } from "./features/batch-import";
import { QueueSchedulerPanel } from "./features/scheduler";
import { EditRequestModal } from "./features/routing";
import { loadSettings, saveSettings, defaultSettings, EdmSettings } from "./types/settings";
import { applyFontFamily } from "./lib/fonts";
import { formatBytes, formatSpeed, formatEta } from "./utils/format";
import { motion, AnimatePresence } from "motion/react";
import { viewVariants, backdropVariants, modalVariants } from "./utils/motion";
import {
  isTauri,
  tauriStartDownload,
  tauriPauseDownload,
  tauriResumeDownload,
  tauriCancelDownload,
  tauriDeleteDownload,
  tauriOpenFile,
  tauriOpenFolder,
  tauriListDownloads,
  tauriSaveSettings,
  tauriLoadSettings,
  tauriGetTelemetry,
  tauriIsWizardCompleted,
  tauriCheckDuplicateDownload,
  tauriSetSpeedLimit,
  tauriOnDragDrop,
  tauriSendSystemNotification,
  tauriCheckForUpdates,
  tauriInstallUpdate,
  tauriOpenInstallerFolder,
  tauriListenOtaProgress,
  tauriGetSystemUserDirs,
  UpdateCheckResult,
} from "./services/tauriBridge";

export interface WorkerInfo {
  idx: number;
  start: number;
  end: number;
  pos: number;
  speed: string;
  status: string;
}

export interface DownloadItem {
  id: string;
  name: string;
  category: "Video" | "Music" | "Documents" | "Compressed" | "Programs" | "Pictures" | "Other";
  sizeTotal: number;
  totalBytes?: number;
  sizeDone: number;
  speed: string;
  eta: string;
  connections: number;
  state: "downloading" | "paused" | "queued" | "completed" | "failed";
  progress: number;
  url: string;
  domain: string;
  createdAt: number;
  dateAdded: string;
  workers?: WorkerInfo[];
  speedHistory?: number[];
  uploadHistory?: number[];
  peakSpeedBps?: number;
  elapsedSeconds?: number;
  uploadedBytes?: number;
  error_msg?: string | null;
}

export type DownloadTask = DownloadItem;

export const generateSpeedSvgPath = (history: number[] = [], peakBps: number = 0) => {
  const VIEW_WIDTH = 100;
  const VIEW_HEIGHT = 44;
  const TOP_PAD = 4;
  const BOTTOM_PAD = 38;
  const USABLE_H = BOTTOM_PAD - TOP_PAD;

  if (!history || history.length === 0) {
    return {
      linePath: `M 0,${BOTTOM_PAD} L ${VIEW_WIDTH},${BOTTOM_PAD}`,
      areaPath: `M 0,${BOTTOM_PAD} L ${VIEW_WIDTH},${BOTTOM_PAD} L ${VIEW_WIDTH},${VIEW_HEIGHT} L 0,${VIEW_HEIGHT} Z`,
    };
  }

  // Ensure sufficient rolling resolution (between 24 and 60 points) for fluid NetPulse spline
  let data = [...history];
  const targetMin = 24;
  if (data.length < targetMin) {
    const firstVal = data[0] || 0;
    data = [...Array(targetMin - data.length).fill(firstVal), ...data];
  } else if (data.length > 60) {
    data = data.slice(-60);
  }

  const maxVal = Math.max(peakBps, ...data, 1024);
  const n = data.length;

  const points = data.map((val, idx) => {
    const x = (idx / (n - 1)) * VIEW_WIDTH;
    const ratio = Math.max(0, Math.min(1, val / maxVal));
    const y = BOTTOM_PAD - ratio * USABLE_H;
    return { x, y };
  });

  if (points.length === 1) {
    return {
      linePath: `M 0,${points[0].y.toFixed(2)} L ${VIEW_WIDTH},${points[0].y.toFixed(2)}`,
      areaPath: `M 0,${points[0].y.toFixed(2)} L ${VIEW_WIDTH},${points[0].y.toFixed(2)} L ${VIEW_WIDTH},${VIEW_HEIGHT} L 0,${VIEW_HEIGHT} Z`,
    };
  }

  // Centripetal Catmull-Rom spline with bounded control points to guarantee smooth, organic curves (NetPulse style)
  let linePath = `M ${points[0].x.toFixed(2)},${points[0].y.toFixed(2)}`;
  for (let i = 0; i < n - 1; i++) {
    const p0 = points[Math.max(0, i - 1)];
    const p1 = points[i];
    const p2 = points[i + 1];
    const p3 = points[Math.min(n - 1, i + 2)];

    const cp1x = p1.x + (p2.x - p0.x) / 6;
    const cp1y = Math.max(TOP_PAD - 1, Math.min(BOTTOM_PAD + 1, p1.y + (p2.y - p0.y) / 6));
    const cp2x = p2.x - (p3.x - p1.x) / 6;
    const cp2y = Math.max(TOP_PAD - 1, Math.min(BOTTOM_PAD + 1, p2.y - (p3.y - p1.y) / 6));

    linePath += ` C ${cp1x.toFixed(2)},${cp1y.toFixed(2)} ${cp2x.toFixed(2)},${cp2y.toFixed(2)} ${p2.x.toFixed(2)},${p2.y.toFixed(2)}`;
  }

  const areaPath = `${linePath} L ${VIEW_WIDTH},${VIEW_HEIGHT} L 0,${VIEW_HEIGHT} Z`;
  return { linePath, areaPath };
};

export default function App() {
  const [activeView, setActiveView] = useState<"downloads" | "settings">(
    () => {
      if (typeof window !== "undefined") {
        if (window.location.hash === "#settings" || window.location.hash === "#queues" || window.location.hash === "#section-queues") return "settings";
      }
      return "downloads";
    }
  );
  const [settings, setSettings] = useState<EdmSettings>(loadSettings);
  const [isWizardOpen, setIsWizardOpen] = useState(false);
  const [refreshModalTask, setRefreshModalTask] = useState<DownloadItem | null>(null);
  const [editRequestTask, setEditRequestTask] = useState<DownloadItem | null>(null);

  useEffect(() => {
    tauriIsWizardCompleted().then((completed) => {
      if (!completed) {
        setIsWizardOpen(true);
      }
    });
  }, []);

  const mode = settings.general.theme === "system" ? "dark" : settings.general.theme;
  const palette = settings.general.palette;

  useEffect(() => {
    applyFontFamily(settings.general.fontFamily);
  }, [settings.general.fontFamily]);

  const handleUpdateSettings = (newSettings: EdmSettings) => {
    setSettings(newSettings);
    saveSettings(newSettings);
    tauriSaveSettings(newSettings);
  };

  const [viewMode, setViewMode] = useState<"table" | "grid">("table");

  // Synchronize hash routing with activeView
  useEffect(() => {
    const handleHash = () => {
      if (window.location.hash === "#settings" || window.location.hash === "#queues" || window.location.hash === "#section-queues") setActiveView("settings");
      else setActiveView("downloads");
    };
    window.addEventListener("hashchange", handleHash);
    return () => window.removeEventListener("hashchange", handleHash);
  }, []);
  const [statusFilter, setStatusFilter] = useState("all");
  const [categoryFilter, setCategoryFilter] = useState("all");
  const [selectedDownload, setSelectedDownload] = useState<DownloadItem | null>(null);
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const [isAddModalOpen, setIsAddModalOpen] = useState(false);
  const [pendingCapture, setPendingCapture] = useState<{
    url: string;
    filename?: string;
    size?: number | null;
  } | null>(null);
  const [isAddSchedulerMode, setIsAddSchedulerMode] = useState(false);
  const [isBatchImportOpen, setIsBatchImportOpen] = useState(false);
  const [isSchedulerOpen, setIsSchedulerOpen] = useState(false);
  const [isInspectorOpen, setIsInspectorOpen] = useState(false);
  const [deleteModalTarget, setDeleteModalTarget] = useState<{ id: string; name: string } | null>(null);
  const [deleteModalDontAsk, setDeleteModalDontAsk] = useState<boolean>(false);
  const [speedLimit, setSpeedLimit] = useState<string>("Unlimited");
  const [updateInfo, setUpdateInfo] = useState<UpdateCheckResult | null>(null);
  const [showUpdateBanner, setShowUpdateBanner] = useState<boolean>(false);
  const [updateBannerState, setUpdateBannerState] = useState<"available" | "downloading" | "installing" | "failed">("available");
  const [updateBannerProgress, setUpdateBannerProgress] = useState<number>(0);
  const [updateBannerError, setUpdateBannerError] = useState<string | null>(null);
  const notifiedCompletedIdsRef = useRef<Set<string>>(new Set());

  const handleSpeedLimitChange = (val: string) => {
    setSpeedLimit(val);
    tauriSetSpeedLimit(SPEED_LIMIT_BYTES_MAP[val] ?? null);
  };

  useEffect(() => {
    if (settings.general.autoCheckUpdates) {
      tauriCheckForUpdates().then((res) => {
        if (res && res.update_available) {
          setUpdateInfo(res);
          const dismissed = localStorage.getItem(DISMISSED_UPDATE_VERSION_KEY);
          if (dismissed !== res.latest_version) {
            setShowUpdateBanner(true);
          }
        }
      }).catch((err) => {
        console.warn("Auto update check failed:", err);
      });
    }
  }, [settings.general.autoCheckUpdates]);

  const handleDismissUpdateBanner = () => {
    if (updateInfo?.latest_version) {
      try {
        localStorage.setItem(DISMISSED_UPDATE_VERSION_KEY, updateInfo.latest_version);
      } catch {}
    }
    setShowUpdateBanner(false);
  };

  const handleBannerUpdate = async () => {
    if (!updateInfo) return;
    if (updateInfo.download_url) {
      setUpdateBannerState("downloading");
      setUpdateBannerProgress(0);
      setUpdateBannerError(null);

      let unlistenFn: (() => void) | null = null;
      try {
        unlistenFn = await tauriListenOtaProgress((progress) => {
          setUpdateBannerProgress(Math.round(progress.percent));
          if (progress.status === "ready" || progress.percent >= 100) {
            setUpdateBannerState("installing");
          }
        });
        await tauriInstallUpdate(updateInfo.download_url);
      } catch (err: any) {
        console.warn("Update installation trigger failed:", err);
        setUpdateBannerState("failed");
        setUpdateBannerError(err?.message || String(err) || "Failed to download update");
        if (unlistenFn) unlistenFn();
      }
    } else {
      setActiveView("settings");
    }
  };

  const triggerCompletionNotification = (filename: string, id: string, filePath?: string) => {
    if (notifiedCompletedIdsRef.current.has(id)) return;
    notifiedCompletedIdsRef.current.add(id);

    // Audio chime if enabled
    if (settings.notifications.playSound) {
      try {
        const AudioCtx = window.AudioContext || (window as any).webkitAudioContext;
        if (AudioCtx) {
          const ctx = new AudioCtx();
          const now = ctx.currentTime;
          const osc = ctx.createOscillator();
          const gain = ctx.createGain();

          osc.type = "sine";
          osc.frequency.setValueAtTime(587.33, now); // D5
          osc.frequency.exponentialRampToValueAtTime(880, now + 0.12); // A5

          gain.gain.setValueAtTime(0.08, now);
          gain.gain.exponentialRampToValueAtTime(0.001, now + 0.45);

          osc.connect(gain);
          gain.connect(ctx.destination);

          osc.start(now);
          osc.stop(now + 0.45);
        }
      } catch {}
    }

    // OS Notification banner if enabled (Native Windows Toast only; no in-app alert)
    if (settings.notifications.downloadCompleted) {
      tauriSendSystemNotification(
        "EDM Download Complete",
        `${filename} has finished downloading.`,
        filePath
      );
    }
  };

  type SortField = "dateAdded" | "name" | "size" | "progress" | "speed";
  const [sortField, setSortField] = useState<SortField>("dateAdded");
  const [sortDirection, setSortDirection] = useState<"asc" | "desc">("desc");

  interface ContextMenuState {
    visible: boolean;
    x: number;
    y: number;
    item: DownloadItem | null;
  }
  const [contextMenu, setContextMenu] = useState<ContextMenuState>({
    visible: false,
    x: 0,
    y: 0,
    item: null,
  });

  const handleSort = (field: SortField) => {
    if (sortField === field) {
      setSortDirection((prev) => (prev === "asc" ? "desc" : "asc"));
    } else {
      setSortField(field);
      setSortDirection("asc");
    }
  };

  const handleContextMenu = (e: React.MouseEvent, item: DownloadItem) => {
    e.preventDefault();
    e.stopPropagation();
    setSelectedDownload(item);
    if (!selectedIds.includes(item.id)) {
      setSelectedIds([item.id]);
    }
    setContextMenu({
      visible: true,
      x: Math.min(e.clientX, window.innerWidth - 230),
      y: Math.min(e.clientY, window.innerHeight - 270),
      item,
    });
  };

  const resumeSelected = () => {
    if (selectedIds.length === 0) return;
    selectedIds.forEach((id) => {
      const item = downloads.find((d) => d.id === id);
      if (item && item.state === "paused") {
        tauriResumeDownload(id);
      }
    });
    setDownloads((prev) =>
      prev.map((d) => (selectedIds.includes(d.id) && d.state === "paused" ? { ...d, state: "downloading" } : d))
    );
  };

  const pauseSelected = () => {
    if (selectedIds.length === 0) return;
    selectedIds.forEach((id) => {
      const item = downloads.find((d) => d.id === id);
      if (item && item.state === "downloading") {
        tauriPauseDownload(id);
      }
    });
    setDownloads((prev) =>
      prev.map((d) => (selectedIds.includes(d.id) && d.state === "downloading" ? { ...d, state: "paused", speed: "0 B/s" } : d))
    );
  };

  const stopSelected = () => {
    if (selectedIds.length === 0) return;
    selectedIds.forEach((id) => {
      const item = downloads.find((d) => d.id === id);
      if (item && item.state === "downloading") {
        tauriCancelDownload(id);
      }
    });
    setDownloads((prev) =>
      prev.map((d) => (selectedIds.includes(d.id) && d.state === "downloading" ? { ...d, state: "paused", speed: "0 B/s" } : d))
    );
  };

  useEffect(() => {
    const handleCloseContext = () => {
      setContextMenu((prev) => (prev.visible ? { ...prev, visible: false } : prev));
    };
    window.addEventListener("click", handleCloseContext);
    return () => {
      window.removeEventListener("click", handleCloseContext);
    };
  }, []);

  // Window drag-and-drop ingestion for .torrent files
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    tauriOnDragDrop(async (paths) => {
      for (const p of paths) {
        if (p.toLowerCase().endsWith(".torrent")) {
          const fname = p.split(/[\\/]/).pop() || "torrent_download";
          await tauriStartDownload({
            url: p,
            filename: fname,
            saveDir: settings.downloads.defaultDirectory,
            maxConnections:
              settings.downloads.segmentedDownloads === "disabled"
                ? 1
                : settings.downloads.autoAdaptiveStreams !== false
                ? 0
                : settings.downloads.maxConnectionsPerFile,
          });
        }
      }
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      if (unlisten) unlisten();
    };
  }, [settings.downloads.defaultDirectory]);

  const [searchQuery, setSearchQuery] = useState("");
  const [isAboutOpen, setIsAboutOpen] = useState(false);
  const [downloads, setDownloads] = useState<DownloadItem[]>(() => {
    if (typeof window !== "undefined") {
      try {
        const saved = localStorage.getItem(DOWNLOADS_STORAGE_KEY);
        if (saved) {
          const parsed = JSON.parse(saved);
          if (Array.isArray(parsed) && (parsed.length > 0 || isTauri())) {
            return parsed.map((item: DownloadItem) => {
              const itemWithCreated: DownloadItem = {
                ...item,
                createdAt: item.createdAt || (item.dateAdded && item.dateAdded !== "Just now" && item.dateAdded !== "Today" ? new Date(item.dateAdded).getTime() : 0) || Date.now(),
              };
              if (item.state === "completed" || item.progress >= 100 || (item.sizeTotal > 0 && item.sizeDone >= item.sizeTotal)) {
                const total = item.sizeTotal || item.sizeDone || 0;
                return {
                  ...itemWithCreated,
                  state: "completed" as const,
                  progress: 100,
                  sizeDone: total,
                  sizeTotal: total,
                  speed: "0 B/s",
                  eta: "Finished",
                  connections: 0,
                  workers: item.workers?.map((w) => ({
                    ...w,
                    pos: w.end,
                    speed: "0 B/s",
                    status: "Chunk verified • Complete",
                  })),
                };
              }
              return itemWithCreated;
            });
          }
        }
      } catch (e) {
        console.warn("Failed to read edm_downloads_v1", e);
      }
    }
    return [];
  });

  // Persist download tasks across sessions
  useEffect(() => {
    try {
      localStorage.setItem(DOWNLOADS_STORAGE_KEY, JSON.stringify(downloads));
    } catch (e) {
      console.warn("Failed to persist edm_downloads_v1", e);
    }
  }, [downloads]);

  // Hydrate settings from SQLite backend on startup if available
  useEffect(() => {
    tauriLoadSettings().then((backendSettings) => {
      if (backendSettings) {
        if (!backendSettings.categories?.some((c) => c.id === "pictures")) {
          const picCat = defaultSettings.categories.find((c) => c.id === "pictures");
          if (picCat) {
            backendSettings.categories = [...(backendSettings.categories || []), picCat];
          }
        }
        setSettings(backendSettings);
      }
    });

    tauriGetSystemUserDirs().then((dirs) => {
      if (!dirs) return;
      const categoryFolderMap = buildCategoryFolderMap(dirs);

      setSettings((current) => {
        let changed = false;
        const downloads = { ...current.downloads };
        if (!downloads.defaultDirectory) {
          downloads.defaultDirectory = dirs.downloads;
          changed = true;
        }
        if (!downloads.temporaryDirectory) {
          downloads.temporaryDirectory = dirs.temp || `${dirs.downloads}\\temp`;
          changed = true;
        }

        const categories = current.categories.map((c) => {
          let dest = c.destinationDirectory || "";
          const catId = c.id.toLowerCase();

          // 1. Fix duplicate \EDM\EDM occurrences
          if (dest.includes("\\EDM\\EDM") || dest.includes("/EDM/EDM")) {
            dest = dest.replace(/([\\/]EDM)+$/gi, "\\EDM");
            changed = true;
          }

          // 2. Audio in Downloads -> migrate to Music\EDM
          if ((catId === "audio" || catId === "music") && (dest.includes("Downloads\\Audio") || dest.includes("Downloads/Audio") || dest.includes("Downloads\\Music") || dest.includes("Downloads/Music"))) {
            dest = ensureSingleEdmPath(dirs.music);
            changed = true;
          }

          // 3. In intelligent categorized mode ("auto"), ensure paths stay strictly mapped to standard folders
          if (current.storage.folderMode === "auto" && categoryFolderMap[catId]) {
            if (dest !== categoryFolderMap[catId]) {
              dest = categoryFolderMap[catId];
              changed = true;
            }
          } else if (!dest) {
            dest = categoryFolderMap[catId] || `${dirs.downloads}\\${c.name}`;
            changed = true;
          }

          return { ...c, destinationDirectory: dest };
        });

        if (changed) {
          const next = { ...current, downloads, categories };
          saveSettings(next);
          tauriSaveSettings(next);
          return next;
        }
        return current;
      });
    });
  }, []);

  // Bidirectional real-time sync with backend SQLite store & extension tasks
  useEffect(() => {
    let mounted = true;

    const syncWithBackend = async () => {
      const backendItems = await tauriListDownloads();
      if (!backendItems || !mounted) return;

      setDownloads((prev) => {
        const existingMap = new Map(prev.map((item) => [item.id, item]));

        const updated: DownloadItem[] = backendItems.map((b) => {
          const existing = existingMap.get(b.id);
          const total = Math.max(b.size_total || 0, existing?.sizeTotal || 0);
          const done = Math.max(b.size_done || 0, existing?.sizeDone || 0);
          const calculatedPct = total > 0 ? Math.min(100, Math.floor((done / total) * 100)) : 0;
          const pct = Math.max(calculatedPct, existing?.progress || 0);
          const speedVal = b.speed_bytes_per_sec || 0;

          const category = VALID_DOWNLOAD_CATEGORIES.includes(b.category as any)
            ? (b.category as DownloadItem["category"])
            : "Programs";

          const domain = (() => {
            try {
              return new URL(b.url).hostname;
            } catch {
              return "web";
            }
          })();

          const rawState = BACKEND_STATE_MAP[b.state] || (existing?.state || "downloading");
          const isCompleted = b.state === "completed" || (rawState === "completed" && pct >= 100);
          if (existing && existing.state !== "completed" && isCompleted) {
            triggerCompletionNotification(b.filename, b.id, b.save_dir ? `${b.save_dir}\\${b.filename}` : undefined);
          }
          const itemState = isCompleted ? ("completed" as const) : rawState;
          const finalTotal = isCompleted && total === 0 && done > 0 ? done : total;
          const finalDone = isCompleted && finalTotal > 0 ? finalTotal : done;
          const isDownloading = itemState === "downloading";
          const currentBps = isDownloading ? speedVal : 0;
          const history = isDownloading
            ? existing?.speedHistory
              ? [...existing.speedHistory.slice(-59), currentBps]
              : [currentBps]
            : existing?.speedHistory && existing.speedHistory.length > 0
              ? existing.speedHistory
              : [0];

          // Single source of truth: preserve live smoothed ETA and connections from telemetry
          const eta = isCompleted
            ? "Finished"
            : itemState === "failed"
            ? (b.error_msg || "Download Failed")
            : itemState === "paused"
            ? "Paused"
            : (existing?.eta && existing.eta !== "—"
                ? existing.eta
                : (b.eta_seconds ? formatEta(b.eta_seconds) : "Calculating..."));

          const isTorrentItem =
            b.url.startsWith("magnet:") ||
            b.url.toLowerCase().includes(".torrent") ||
            b.filename.toLowerCase().endsWith(".torrent") ||
            category.toLowerCase() === "torrents";
          const finalWorkers = isCompleted && existing?.workers
            ? existing.workers.map((w) => ({
                ...w,
                pos: w.end,
                speed: "0 B/s",
                status: isTorrentItem ? "Verified • Seeding" : "Chunk verified • Complete",
              }))
            : existing?.workers;

          return {
            id: b.id,
            name: b.filename,
            category,
            sizeTotal: finalTotal,
            totalBytes: finalTotal,
            sizeDone: finalDone,
            error_msg: b.error_msg || existing?.error_msg || null,
            speed: isDownloading ? (existing?.speed || formatSpeed(speedVal)) : "0 B/s",
            eta,
            connections: isDownloading ? (existing?.connections || b.connections || 1) : 0,
            state: itemState,
            progress: isCompleted ? 100 : pct,
            url: b.url,
            domain,
            createdAt: b.created_at ? new Date(b.created_at).getTime() : (existing?.createdAt || Date.now()),
            dateAdded: b.created_at ? new Date(b.created_at).toLocaleDateString() : (existing?.dateAdded || "Today"),
            workers: finalWorkers,
            speedHistory: history,
            uploadHistory: existing?.uploadHistory || [],
            uploadedBytes: (b as any).uploaded_bytes || existing?.uploadedBytes || 0,
            peakSpeedBps: Math.max(existing?.peakSpeedBps || 0, speedVal),
            elapsedSeconds: existing?.elapsedSeconds || 0,
          };
        });

        // Also keep any local-only preview items that haven't hit backend yet
        const backendIds = new Set(backendItems.map((b) => b.id));
        for (const item of prev) {
          if (!backendIds.has(item.id) && Date.now() - (item.createdAt || Number(item.id) || 0) < LOCAL_PREVIEW_GRACE_MS) {
            updated.push(item);
          }
        }

        return updated;
      });
    };

    // Initial immediate sync
    syncWithBackend();

    // Listen for backend downloads-changed and prompt-add-download events
    let unlisten: (() => void) | undefined;
    let unlistenPrompt: (() => void) | undefined;
    if (isTauri()) {
      import("@tauri-apps/api/event").then(({ listen }) => {
        try {
          listen("downloads-changed", () => {
            syncWithBackend();
          }).then((fn) => {
            if (mounted) unlisten = fn;
            else fn();
          }).catch(() => {});

          listen<{ url: string; filename?: string; size?: number | null }>(
            "prompt-add-download",
            (event) => {
              if (!event.payload?.url) return;
              setPendingCapture({
                url: event.payload.url,
                filename: event.payload.filename,
                size: event.payload.size,
              });
              setIsAddSchedulerMode(false);
              setIsAddModalOpen(true);
            }
          ).then((fn) => {
            if (mounted) unlistenPrompt = fn;
            else fn();
          }).catch(() => {});
        } catch {}
      }).catch(() => {});
    }

    // Fast sync interval
    const interval = setInterval(syncWithBackend, BACKEND_SYNC_INTERVAL_MS);
    return () => {
      mounted = false;
      clearInterval(interval);
      if (unlisten) unlisten();
      if (unlistenPrompt) unlistenPrompt();
    };
  }, []);

  // Synchronize mode and palette attributes on root document, and pause idle animations when unfocused
  useEffect(() => {
    document.documentElement.setAttribute("data-mode", mode);
    document.documentElement.setAttribute("data-palette", palette);
    document.documentElement.setAttribute("data-theme", mode);

    const onBlur = () => document.documentElement.classList.add("window-unfocused");
    const onFocus = () => document.documentElement.classList.remove("window-unfocused");
    window.addEventListener("blur", onBlur);
    window.addEventListener("focus", onFocus);
    return () => {
      window.removeEventListener("blur", onBlur);
      window.removeEventListener("focus", onFocus);
    };
  }, [mode, palette]);





  const handleAddDownload = async (
    customUrl?: string,
    customFilename?: string,
    customCategory?: string,
    customConns?: number,
    customSaveDir?: string,
    selectedFiles?: number[],
    queueOnly?: boolean
  ) => {
    const targetUrl = customUrl || "";
    if (!targetUrl.trim()) return;
    const isYouTubeUrl = targetUrl.includes("youtube.com") || targetUrl.includes("youtu.be");
    let initialName = (customFilename && customFilename.trim()) || targetUrl.split("/").pop()?.split("?")[0] || "download.bin";
    if (isYouTubeUrl && (!customFilename || !customFilename.trim() || initialName === "watch" || initialName === "watch.mp4")) {
      initialName = "";
    }
    const name = initialName ? initialName.replace(/[\\/:*?"<>|]/g, "_") : (isYouTubeUrl ? "YouTube_Video.mp4" : "download.bin");
    const ext = name.split(".").pop()?.toLowerCase() || "";
    const categoryName = inferCategoryFromExtension(ext, settings.categories);

    let hostname = "source.net";
    try {
      hostname = new URL(targetUrl.startsWith("http") ? targetUrl : `https://${targetUrl}`).hostname;
    } catch {
      hostname = "download.net";
    }

    const dupCheck = await tauriCheckDuplicateDownload(
      targetUrl,
      name,
      0
    );
    if (dupCheck.is_duplicate) {
      const proceed = confirm(
        `Duplicate Download Detected:\n${dupCheck.existing_filename ? `File already exists as ${dupCheck.existing_filename} (${dupCheck.existing_state})` : "This download already exists in tasks."}\n\nDo you still wish to add it again?`
      );
      if (!proceed) return;
    }

    const isSingleStream =
      customConns === 1 ||
      (customConns === undefined &&
        (settings.downloads.segmentedDownloads === "disabled" ||
          (settings.downloads.autoAdaptiveStreams === false &&
            settings.downloads.maxConnectionsPerFile === 1)));
    const ceilingConns = isSingleStream
      ? 1
      : customConns && customConns > 0
      ? customConns
      : settings.downloads.maxConnectionsPerFile || DEFAULT_MAX_CONNECTIONS;
    const initialConns = isSingleStream ? 1 : Math.min(4, ceilingConns);
    const backendMaxConnections =
      customConns !== undefined
        ? customConns
        : settings.downloads.segmentedDownloads === "disabled"
        ? 1
        : settings.downloads.autoAdaptiveStreams !== false
        ? 0
        : settings.downloads.maxConnectionsPerFile;
    const finalCategory = (customCategory && customCategory !== "auto") ? (customCategory as DownloadItem["category"]) : categoryName;

    // Intelligent Categorization: if user didn't explicitly pick a custom save directory,
    // route into the matching category's destinationDirectory!
    let targetDir = (customSaveDir && customSaveDir.trim()) || "";
    if (!targetDir || targetDir === settings.downloads.defaultDirectory) {
      if (settings.storage.folderMode === "auto") {
        const catRule = settings.categories.find((c) =>
          c.enabled && (
            c.name.toLowerCase() === finalCategory.toLowerCase() ||
            c.id.toLowerCase() === finalCategory.toLowerCase() ||
            (finalCategory === "Compressed" && (c.id === "archives" || c.name === "Archives")) ||
            (finalCategory === "Music" && (c.id === "audio" || c.name === "Audio"))
          )
        );
        if (catRule && catRule.destinationDirectory) {
          targetDir = catRule.destinationDirectory;
        }
      }
    }
    if (!targetDir) {
      targetDir = settings.downloads.defaultDirectory;
    }

    const backendRes = await tauriStartDownload({
      url: targetUrl,
      filename: name,
      saveDir: targetDir,
      maxConnections: backendMaxConnections,
      selectedFiles,
      queueOnly,
    });

    const isQueued = backendRes?.status === "queued";
    const realTotal = DEFAULT_FALLBACK_SIZE_BYTES;

    const initialWorkers: WorkerInfo[] = isQueued
      ? []
      : Array.from({ length: initialConns }).map((_, idx) => {
          const chunkBytes = Math.floor(realTotal / initialConns);
          const start = idx * chunkBytes;
          const end = idx === initialConns - 1 ? realTotal : (idx + 1) * chunkBytes;
          return {
            idx: idx + 1,
            start,
            end,
            pos: start,
            speed: "0 B/s",
            status: "Connecting socket...",
          };
        });

    const newItem: DownloadItem = {
      id: backendRes.id || Date.now().toString(),
      name,
      category: finalCategory,
      sizeTotal: realTotal,
      sizeDone: 0,
      speed: "0 B/s",
      eta: isQueued ? "Queued" : "Connecting...",
      connections: isQueued ? 0 : initialConns,
      state: isQueued ? "queued" : "downloading",
      progress: 0,
      url: targetUrl,
      domain: hostname,
      createdAt: Date.now(),
      dateAdded: "Just now",
      workers: initialWorkers,
      speedHistory: [0],
      uploadHistory: [],
      uploadedBytes: 0,
      peakSpeedBps: 0,
      elapsedSeconds: 0,
    };
    setDownloads([newItem, ...downloads]);
    setSelectedDownload(newItem);
    setSelectedIds([newItem.id]);
    setIsAddModalOpen(false);
  };

  const downloadsRef = useRef(downloads);
  useEffect(() => {
    downloadsRef.current = downloads;
  }, [downloads]);

  interface TelemetrySmoothingState {
    lastValidEta: string;
    lastProgressTime: number;
    speedSamples: number[];
    elapsedSeconds: number;
    lastUiUpdate: number;
    displayedEta: string;
    displayedSpeed: string;
    displayedConns: number;
  }
  const telemetrySmoothingRef = useRef<Map<string, TelemetrySmoothingState>>(new Map());

  const hasDownloadingTasks = downloads.some((d) => d.state === "downloading");

  // Active Telemetry and Real Progress Tick (250 ms Monotonic Clock Sampling)
  useEffect(() => {
    if (!hasDownloadingTasks) return;

    const interval = setInterval(async () => {
      const activeTasks = downloadsRef.current.filter((d) => d.state === "downloading");
      if (activeTasks.length === 0) return;
      const updates = await Promise.all(
        activeTasks.map(async (task) => {
          const telem = await tauriGetTelemetry(task.id);
          let newWorkers: WorkerInfo[] = [];

          let smooth = telemetrySmoothingRef.current.get(task.id);
          if (!smooth) {
            smooth = {
              lastValidEta: task.eta || "Calculating...",
              lastProgressTime: Date.now(),
              speedSamples: [],
              elapsedSeconds: task.elapsedSeconds || 0,
              lastUiUpdate: 0,
              displayedEta: task.eta || "Calculating...",
              displayedSpeed: task.speed || "0 B/s",
              displayedConns: task.connections || 8,
            };
            telemetrySmoothingRef.current.set(task.id, smooth);
          }

          if (telem) {
            const phase = (telem as any).phase;
            const rawDone = telem.downloaded_bytes;
            const effectiveTotal = telem.total_bytes && telem.total_bytes > 0
              ? telem.total_bytes
              : (phase === "Resolving" ? 0 : task.sizeTotal);
            let calculatedPct = 0;
            if (effectiveTotal > 0) {
              calculatedPct = Math.min(100, Math.floor((rawDone / effectiveTotal) * 100));
            }
            const rawProgress = Math.max(calculatedPct, Math.min(100, Math.floor(telem.progress_pct || 0)));
            const isFinished = ((rawProgress >= 100 || (effectiveTotal > 0 && rawDone >= effectiveTotal)) && phase !== "Merging" && phase !== "Resolving") || phase === "Completed";
            const sizeTotal = effectiveTotal > 0
              ? effectiveTotal
              : (rawDone > 0 && rawProgress > 0 ? Math.round((rawDone / (rawProgress / 100))) : 0);
            const sizeDone = isFinished && sizeTotal > 0 ? sizeTotal : rawDone;
            const progress = isFinished ? 100 : (phase === "Merging" ? 95 : (phase === "Resolving" ? 0 : rawProgress));
            const now = Date.now();

            if (!isFinished) {
              smooth.elapsedSeconds += 0.25;
            } else if (smooth.elapsedSeconds === 0 && (task.elapsedSeconds || 0) > 0) {
              smooth.elapsedSeconds = task.elapsedSeconds || 0;
            }

            // Speed smoothing EMA (8s window = 32 samples @ 250ms)
            const instantSpeed = Math.floor((telem.instant_throughput_bps || telem.rolling_throughput_bps) / 8);
            if (instantSpeed > 0) {
              smooth.lastProgressTime = now;
            }
            smooth.speedSamples = [...smooth.speedSamples.slice(-31), instantSpeed];
            const smoothedSpeedBps = Math.round(
              smooth.speedSamples.reduce((a, b) => a + b, 0) / smooth.speedSamples.length
            );

            // Phase and ETA stability logic (AdaptiveTelemetry is single source of truth)
            const isWarmup = smooth.elapsedSeconds < 3 && sizeDone < 1024 * 1024;
            let currentEta = smooth.lastValidEta || "Calculating...";

            if (isFinished) {
              currentEta = "Finished";
              smooth.lastValidEta = "Finished";
            } else if (phase === "Verifying" || telem.decision_reason?.toLowerCase().includes("verif")) {
              smooth.lastProgressTime = now;
              currentEta = telem.decision_reason || "Verifying local files...";
              smooth.lastValidEta = currentEta;
            } else if (phase === "Resolving" || telem.decision_reason?.toLowerCase().includes("fetching video info") || telem.decision_reason?.toLowerCase().includes("resolv")) {
              smooth.lastProgressTime = now;
              currentEta = telem.decision_reason || "Resolving metadata...";
              smooth.lastValidEta = currentEta;
            } else if (phase === "Merging" || telem.decision_reason?.toLowerCase().includes("merg")) {
              currentEta = "Merging...";
              smooth.lastValidEta = "Merging...";
            } else if (isWarmup) {
              currentEta = "Calculating...";
            } else if (telem.eta_seconds !== null && telem.eta_seconds !== undefined && telem.eta_seconds > 0) {
              currentEta = formatEta(telem.eta_seconds);
              smooth.lastValidEta = currentEta;
            } else if (smoothedSpeedBps > 0 && sizeTotal > sizeDone) {
              const rawSec = Math.ceil((sizeTotal - sizeDone) / smoothedSpeedBps);
              currentEta = formatEta(rawSec);
              smooth.lastValidEta = currentEta;
            } else {
              // Speed is zero: only show Stalled if zero bytes for >= 10s during active download
              const stalledMs = now - smooth.lastProgressTime;
              if (phase === "Verifying" || telem.decision_reason?.toLowerCase().includes("verif")) {
                smooth.lastProgressTime = now;
                currentEta = telem.decision_reason || "Verifying local files...";
              } else if (phase === "Resolving" || telem.decision_reason?.toLowerCase().includes("resolv")) {
                currentEta = telem.decision_reason || "Fetching video info...";
              } else if (phase === "Merging") {
                currentEta = "Merging...";
              } else if (stalledMs < STALLED_THRESHOLD_MS && smooth.lastValidEta && smooth.lastValidEta !== "Calculating...") {
                currentEta = smooth.lastValidEta;
              } else if (stalledMs >= STALLED_THRESHOLD_MS && (phase === "DownloadingVideo" || phase === "DownloadingAudio" || phase === "Downloading" || !phase)) {
                currentEta = "Stalled";
              }
            }

            const isVerifying = phase === "Verifying" || telem.decision_reason?.toLowerCase().includes("verif");
            const targetConns = isFinished
              ? 0
              : isVerifying
              ? 0
              : (telem.active_connections !== undefined ? telem.active_connections : (telem.target_connections || 1));

            // Throttle UI label updates to at most 1-2 per second (500ms)
            if (now - smooth.lastUiUpdate >= UI_THROTTLE_MS || isFinished) {
              smooth.displayedEta = currentEta;
              smooth.displayedSpeed = isFinished ? "0 B/s" : formatSpeed(smoothedSpeedBps);
              smooth.displayedConns = targetConns;
              smooth.lastUiUpdate = now;
            }

            const isTorrent =
              task.url.startsWith("magnet:") ||
              task.url.toLowerCase().includes(".torrent") ||
              task.name.toLowerCase().endsWith(".torrent") ||
              task.category?.toLowerCase() === "torrents";

            if (telem.workers && telem.workers.length > 0) {
              const rawWorkerBpsSum = telem.workers.reduce((acc, w) => acc + (w.speed_bps || 0), 0);
              const activeCount = telem.workers.filter(
                (w) => w.status === "downloading" || w.status === "connecting"
              ).length || telem.workers.length;

              newWorkers = telem.workers.map((w) => {
                let workerSpeedBps = 0;
                if (!isFinished && smoothedSpeedBps > 0) {
                  if (rawWorkerBpsSum > 0) {
                    workerSpeedBps = Math.round(smoothedSpeedBps * ((w.speed_bps || 0) / rawWorkerBpsSum));
                  } else if (w.status === "downloading" || w.status === "connecting") {
                    workerSpeedBps = Math.floor(smoothedSpeedBps / activeCount);
                  }
                }

                let workerStatusStr = w.status;
                if (isFinished || w.status === "completed") {
                  workerStatusStr = isTorrent ? "Verified • Seeding" : "Chunk verified • Complete";
                } else if (w.status === "downloading") {
                  workerStatusStr = isTorrent ? "Active swarm exchange" : "Receiving chunk (HTTP 206)...";
                } else if (w.status === "stalled") {
                  workerStatusStr = "Worker stalled • Reconnecting...";
                } else if (w.status === "connecting") {
                  workerStatusStr = isTorrent ? "Contacting peer..." : "Connecting socket...";
                }

                return {
                  idx: w.idx,
                  start: w.start,
                  end: w.end,
                  pos: isFinished ? w.end : w.pos,
                  speed: isFinished ? "0 B/s" : formatSpeed(workerSpeedBps),
                  status: workerStatusStr,
                };
              });
            } else if (isFinished && task.workers && task.workers.length > 0) {
              newWorkers = task.workers.map((w) => ({
                ...w,
                pos: w.end,
                speed: "0 B/s",
                status: isTorrent ? "Verified • Seeding" : "Chunk verified • Complete",
              }));
            } else {
              const numConns = Math.max(1, smooth.displayedConns);
              const chunkBytes = Math.floor(sizeTotal / numConns);
              newWorkers = Array.from({ length: numConns }).map((_, idx) => {
                const start = idx * chunkBytes;
                const end = idx === numConns - 1 ? sizeTotal : (idx + 1) * chunkBytes;
                const rangeSize = end - start;
                const workerBytes = Math.min(rangeSize, Math.floor((rangeSize * progress) / 100));
                return {
                  idx: idx + 1,
                  start,
                  end,
                  pos: start + workerBytes,
                  speed: formatSpeed(Math.floor(smoothedSpeedBps / numConns)),
                  status: isFinished
                    ? isTorrent ? "Verified • Seeding" : "Chunk verified • Complete"
                    : isTorrent ? "Active swarm exchange" : "Receiving chunk (HTTP 206)...",
                };
              });
            }

            const prevHistory = task.speedHistory || [];
            const hasNonZeroHistory = prevHistory.some((v) => v > 0);
            const history = isFinished
              ? hasNonZeroHistory
                ? prevHistory
                : smoothedSpeedBps > 0
                  ? [...prevHistory.slice(-59), smoothedSpeedBps]
                  : prevHistory.length > 0
                    ? prevHistory
                    : [0]
              : [...prevHistory.slice(-59), smoothedSpeedBps];
            const peak = Math.max(task.peakSpeedBps || 0, smoothedSpeedBps);
            const uploadBps = Math.floor((telem.upload_throughput_bps || 0) / 8);
            const ulHistory = [...(task.uploadHistory || []).slice(-59), uploadBps];
            const uploadedBytes = (telem as any).uploaded_bytes ?? (task as any).uploadedBytes ?? 0;

            // Trigger alert notification immediately upon reaching completion
            if (isFinished && !notifiedCompletedIdsRef.current.has(task.id)) {
              triggerCompletionNotification(task.name, task.id);
            }

            const finalElapsedSeconds = isFinished
              ? (task.elapsedSeconds && task.elapsedSeconds > 0 ? task.elapsedSeconds : Math.floor(smooth.elapsedSeconds))
              : Math.floor(smooth.elapsedSeconds);

            return {
              id: task.id,
              sizeDone,
              sizeTotal,
              totalBytes: sizeTotal,
              progress,
              speed: smooth.displayedSpeed,
              eta: smooth.displayedEta,
              connections: smooth.displayedConns,
              state: isFinished ? ("completed" as const) : ("downloading" as const),
              workers: newWorkers,
              speedHistory: history,
              uploadHistory: ulHistory,
              peakSpeedBps: peak,
              elapsedSeconds: finalElapsedSeconds || task.elapsedSeconds || 0,
              uploadedBytes,
            };
          } else {
            if (task.state === "downloading") {
              smooth.elapsedSeconds += 0.25;
            }
            return {
              id: task.id,
              elapsedSeconds: Math.floor(smooth.elapsedSeconds) || task.elapsedSeconds || 0,
            };
          }
        })
      );

      setDownloads((prev) =>
        prev.map((d) => {
          const match = updates.find((u) => u.id === d.id);
          return match ? { ...d, ...match } : d;
        })
      );

      setSelectedDownload((prev) => {
        if (!prev) return null;
        const match = updates.find((u) => u.id === prev.id);
        return match ? { ...prev, ...match } : prev;
      });
    }, TELEMETRY_TICK_INTERVAL_MS);

    return () => clearInterval(interval);
  }, [hasDownloadingTasks]);

  const toggleSelect = (id: string, e: React.MouseEvent) => {
    e.stopPropagation();
    if (selectedIds.includes(id)) {
      const next = selectedIds.filter((item) => item !== id);
      setSelectedIds(next);
      if (selectedDownload?.id === id) {
        const remaining = downloads.find((d) => next.includes(d.id)) || null;
        setSelectedDownload(remaining);
      }
    } else {
      setSelectedIds([...selectedIds, id]);
      const item = downloads.find((d) => d.id === id);
      if (item) setSelectedDownload(item);
    }
  };

  const selectRow = (item: DownloadItem) => {
    setSelectedDownload(item);
    setSelectedIds([item.id]);
  };

  const toggleDownloadState = (id: string, e?: React.MouseEvent) => {
    if (e) e.stopPropagation();
    setDownloads((prev) =>
      prev.map((d) => {
        if (d.id === id) {
          // If already completed, safely open the downloaded file rather than re-downloading
          if (d.state === "completed") {
            tauriOpenFile(id);
            return d;
          }
          const nextState = d.state === "downloading" ? "paused" : "downloading";
          if (nextState === "paused") {
            tauriPauseDownload(id);
          } else {
            tauriResumeDownload(id);
          }
          return {
            ...d,
            state: nextState,
            speed: nextState === "downloading" ? d.speed : "0 B/s",
          };
        }
        return d;
      })
    );
    setSelectedDownload((prev) => {
      if (!prev || prev.id !== id) return prev;
      if (prev.state === "completed") return prev;
      const nextState = prev.state === "downloading" ? "paused" : "downloading";
      return {
        ...prev,
        state: nextState,
        speed: nextState === "downloading" ? prev.speed : "0 B/s",
      };
    });
  };

  const handleRequestDelete = (id: string, name: string) => {
    const pref = settings.general?.deletePreference || "always_ask";
    if (pref === "list_only") {
      tauriDeleteDownload(id, false);
      setDownloads((prev) => prev.filter((d) => d.id !== id));
      setSelectedIds((prev) => prev.filter((item) => item !== id));
      if (selectedDownload?.id === id) setSelectedDownload(null);
    } else if (pref === "delete_file") {
      tauriDeleteDownload(id, true);
      setDownloads((prev) => prev.filter((d) => d.id !== id));
      setSelectedIds((prev) => prev.filter((item) => item !== id));
      if (selectedDownload?.id === id) setSelectedDownload(null);
    } else {
      setDeleteModalDontAsk(false);
      setDeleteModalTarget({ id, name });
    }
  };

  const handleConfirmDelete = (deleteFile: boolean) => {
    if (!deleteModalTarget) return;
    const { id } = deleteModalTarget;
    tauriDeleteDownload(id, deleteFile);
    setDownloads((prev) => prev.filter((d) => d.id !== id));
    setSelectedIds((prev) => prev.filter((item) => item !== id));
    if (selectedDownload?.id === id) setSelectedDownload(null);

    if (deleteModalDontAsk) {
      const newPref = deleteFile ? "delete_file" : "list_only";
      const updatedSettings: EdmSettings = {
        ...settings,
        general: {
          ...settings.general,
          deletePreference: newPref,
        },
      };
      setSettings(updatedSettings);
      saveSettings(updatedSettings);
      tauriSaveSettings(updatedSettings).catch(() => {});
    }

    setDeleteModalTarget(null);
    setDeleteModalDontAsk(false);
  };

  const deleteSelected = () => {
    if (selectedIds.length === 0) return;
    if (selectedIds.length === 1) {
      const item = downloads.find((d) => d.id === selectedIds[0]);
      if (item) {
        handleRequestDelete(item.id, item.name);
        return;
      }
    }
    const pref = settings.general?.deletePreference || "always_ask";
    const deleteFile = pref === "delete_file";
    selectedIds.forEach((id) => tauriDeleteDownload(id, deleteFile));
    setDownloads((prev) => prev.filter((d) => !selectedIds.includes(d.id)));
    setSelectedIds([]);
    setSelectedDownload(null);
  };

  const filteredDownloads = useMemo(() => {
    const q = searchQuery.toLowerCase().trim();
    const list = downloads.filter((item) => {
      const matchesSearch =
        !q ||
        item.name.toLowerCase().includes(q) ||
        item.domain.toLowerCase().includes(q) ||
        item.category.toLowerCase().includes(q) ||
        item.url.toLowerCase().includes(q);
      const matchesStatus =
        statusFilter === "all" ||
        (statusFilter === "downloading" && item.state === "downloading") ||
        (statusFilter === "completed" && item.state === "completed") ||
        (statusFilter === "paused" && item.state === "paused") ||
        (statusFilter === "queued" && item.state === "queued") ||
        (statusFilter === "failed" && item.state === "failed");
      const matchesCategory =
        categoryFilter === "all" || item.category.toLowerCase() === categoryFilter.toLowerCase();
      return matchesSearch && matchesStatus && matchesCategory;
    });

    return [...list].sort((a, b) => {
      let cmp = 0;
      if (sortField === "dateAdded") {
        const cmp = (b.createdAt || 0) - (a.createdAt || 0);
        return sortDirection === "asc" ? -cmp : cmp;
      } else if (sortField === "name") {
        cmp = a.name.localeCompare(b.name);
      } else if (sortField === "size") {
        cmp = (a.sizeTotal || a.sizeDone) - (b.sizeTotal || b.sizeDone);
      } else if (sortField === "progress") {
        cmp = a.progress - b.progress;
      } else if (sortField === "speed") {
        const speedA = a.state === "downloading" ? (a.peakSpeedBps || 0) : 0;
        const speedB = b.state === "downloading" ? (b.peakSpeedBps || 0) : 0;
        cmp = speedA - speedB;
      } else {
        cmp = (b.createdAt || 0) - (a.createdAt || 0);
      }
      return sortDirection === "asc" ? cmp : -cmp;
    });
  }, [downloads, searchQuery, statusFilter, categoryFilter, sortField, sortDirection]);

  const activeSelectedDownload = downloads.find((d) => d.id === selectedDownload?.id) || selectedDownload;

  // UI-02: Comprehensive Keyboard shortcuts (Ctrl+N, Ctrl+,, Esc, Space, Delete, Enter, Ctrl+A)
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      const isInput =
        target &&
        (target.tagName === "INPUT" ||
          target.tagName === "TEXTAREA" ||
          target.isContentEditable);

      if ((e.ctrlKey || e.metaKey) && e.key === "n") {
        e.preventDefault();
        setIsAddModalOpen(true);
        return;
      }
      if ((e.ctrlKey || e.metaKey) && e.key === ",") {
        e.preventDefault();
        setActiveView((v) => (v === "settings" ? "downloads" : "settings"));
        return;
      }
      if (e.key === "Escape") {
        setContextMenu((p) => ({ ...p, visible: false }));
        if (deleteModalTarget) {
          setDeleteModalTarget(null);
          setDeleteModalDontAsk(false);
          return;
        }
        if (activeView === "settings") {
          setActiveView("downloads");
          return;
        }
        setIsAddModalOpen(false);
        setIsInspectorOpen(false);
        setSelectedDownload(null);
        setSelectedIds([]);
        return;
      }

      if (isInput) return;

      if ((e.ctrlKey || e.metaKey) && (e.key === "a" || e.key === "A")) {
        e.preventDefault();
        setSelectedIds(filteredDownloads.map((d) => d.id));
        return;
      }

      if (e.key === " " || e.code === "Space") {
        e.preventDefault();
        const targetId = activeSelectedDownload?.id || selectedIds[0];
        if (targetId) {
          toggleDownloadState(targetId);
        }
        return;
      }

      if (e.key === "Delete") {
        e.preventDefault();
        if (selectedIds.length > 0) {
          deleteSelected();
        } else if (activeSelectedDownload) {
          handleRequestDelete(activeSelectedDownload.id, activeSelectedDownload.name);
        }
        return;
      }

      if (e.key === "Enter") {
        if (activeSelectedDownload) {
          e.preventDefault();
          if (activeSelectedDownload.state === "completed" || activeSelectedDownload.progress >= 100) {
            tauriOpenFile(activeSelectedDownload.id);
          } else {
            setIsInspectorOpen(true);
          }
        }
        return;
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [activeView, activeSelectedDownload, selectedIds, filteredDownloads]);

  // Real aggregate network telemetry calculations
  const activeDownloadsList = downloads.filter((d) => d.state === "downloading");
  const totalActiveThreads = activeDownloadsList.reduce((sum, d) => sum + (d.connections || 0), 0);
  const totalSessionBytes = downloads.reduce((sum, d) => sum + (d.sizeDone || 0), 0);

  let totalSpeedDisplay = "0 B/s";
  let totalUploadSpeedDisplay = "0 B/s";
  if (activeDownloadsList.length > 0) {
    let totalSpeedBps = 0;
    let totalUploadBps = 0;
    for (const d of activeDownloadsList) {
      const match = d.speed.match(/([\d.]+)\s*([KMG]?B\/s)/i);
      if (match) {
        const val = parseFloat(match[1]);
        const unit = match[2].toUpperCase();
        if (unit.startsWith("G")) totalSpeedBps += val * 1024 * 1024 * 1024;
        else if (unit.startsWith("M")) totalSpeedBps += val * 1024 * 1024;
        else if (unit.startsWith("K")) totalSpeedBps += val * 1024;
        else totalSpeedBps += val;
      }
      if (d.uploadHistory && d.uploadHistory.length > 0) {
        totalUploadBps += d.uploadHistory[d.uploadHistory.length - 1] || 0;
      }
    }
    totalSpeedDisplay = formatSpeed(totalSpeedBps);
    totalUploadSpeedDisplay = formatSpeed(totalUploadBps);
  }

  const getCategoryIcon = (category: string) => {
    switch (category.toLowerCase()) {
      case "video":
        return <Film className="w-4 h-4 text-purple-400" />;
      case "music":
      case "audio":
        return <Music className="w-4 h-4 text-amber-400" />;
      case "compressed":
      case "archives":
        return <Archive className="w-4 h-4 text-orange-400" />;
      case "documents":
        return <FileText className="w-4 h-4 text-emerald-400" />;
      case "pictures":
      case "images":
        return <Image className="w-4 h-4 text-cyan-400" />;
      case "programs":
      default:
        return <FileCode className="w-4 h-4 text-[var(--accent)]" />;
    }
  };

  const getStatusBadge = (state: string, errorMsg?: string | null) => {
    switch (state) {
      case "downloading":
        return (
          <span
            className="inline-flex items-center justify-center w-5 h-5"
            title="Downloading"
          >
            <span className="w-2.5 h-2.5 rounded-full bg-[var(--accent)] animate-pulse shadow-[0_0_8px_var(--accent-glow)]" />
          </span>
        );
      case "completed":
        return (
          <span className="inline-flex items-center justify-center w-5 h-5 text-emerald-400" title="Completed">
            <CheckCircle2 className="w-4 h-4" />
          </span>
        );
      case "paused":
        return (
          <span className="inline-flex items-center justify-center w-5 h-5 text-amber-400" title="Paused">
            <Pause className="w-4 h-4" />
          </span>
        );
      case "failed":
        if (errorMsg?.toLowerCase().includes("expire") || errorMsg?.includes("401") || errorMsg?.includes("403") || errorMsg?.includes("410")) {
          return (
            <span className="inline-flex items-center justify-center w-5 h-5 text-amber-400" title="Download Link Expired - Click Refresh">
              <RotateCcw className="w-4 h-4" />
            </span>
          );
        }
        return (
          <span className="inline-flex items-center justify-center w-5 h-5 text-rose-400" title={errorMsg || "Download Failed"}>
            <X className="w-4 h-4" />
          </span>
        );
      case "link_expired":
        return (
          <span className="inline-flex items-center justify-center w-5 h-5 text-amber-400" title="Download Link Expired - Click Refresh">
            <RotateCcw className="w-4 h-4" />
          </span>
        );
      default:
        return (
          <span className="inline-flex items-center justify-center w-5 h-5 text-zinc-400" title="Queued">
            <Clock className="w-4 h-4" />
          </span>
        );
    }
  };

  return (
    <div className="flex flex-col h-screen w-screen overflow-hidden text-sm bg-[var(--bg-chassis)] text-[var(--text-primary)]">
      {/* ─────────────────────────────────────────────────────────────
          1. PERSISTENT WINDOW HEADER & TOP COMMAND DECK
      ────────────────────────────────────────────────────────────── */}
      <AppToolbar
        activeView={activeView}
        onBackToDownloads={() => {
          setActiveView("downloads");
          window.location.hash = "";
        }}
        selectedCount={selectedIds.length}
        onResumeSelected={resumeSelected}
        onPauseSelected={pauseSelected}
        onStopSelected={stopSelected}
        onDeleteSelected={deleteSelected}
        onAddUrl={() => {
          setIsAddSchedulerMode(false);
          setIsAddModalOpen(true);
        }}
        onAddToScheduler={() => {
          setIsAddSchedulerMode(true);
          setIsAddModalOpen(true);
        }}
        searchQuery={searchQuery}
        onSearchChange={setSearchQuery}
        viewMode={viewMode}
        onViewModeChange={setViewMode}
      />

      <AnimatePresence mode="wait">
        {activeView === "settings" ? (
          <motion.div
            key="settings-view"
            variants={viewVariants}
            initial="initial"
            animate="animate"
            exit="exit"
            className="flex-1 flex flex-col min-h-0 overflow-hidden w-full"
          >
            <SettingsPage
              settings={settings}
              onUpdateSettings={handleUpdateSettings}
              onBack={() => {
                setActiveView("downloads");
                window.location.hash = "";
              }}
              speedLimit={speedLimit}
              onSpeedLimitChange={handleSpeedLimitChange}
              selectedDownload={activeSelectedDownload}
              totalSpeed={totalSpeedDisplay}
              totalUploadSpeed={totalUploadSpeedDisplay}
              activeThreads={totalActiveThreads}
              totalSessionBytes={totalSessionBytes}
            />
          </motion.div>
        ) : (
          <motion.div
            key="downloads-view"
            variants={viewVariants}
            initial="initial"
            animate="animate"
            exit="exit"
            className="flex-1 flex flex-col min-h-0 overflow-hidden w-full"
          >
      {/* ─────────────────────────────────────────────────────────────
          2. WORKSPACE BODY: SIDEBAR + MAIN CONTENT AREA
      ────────────────────────────────────────────────────────────── */}
      <div className="flex flex-1 min-h-0">
        {/* Left Navigation Spine (Blended Edge matching Upper Header) */}
        <aside className="w-14 sidebar-3d flex flex-col shrink-0 select-none py-3 z-20">
          <div className="space-y-1 w-full flex flex-col items-center">
            {[
              { id: "all", label: "All Downloads", count: downloads.length, icon: Download },
              {
                id: "completed",
                label: "Completed",
                count: downloads.filter((d) => d.state === "completed").length,
                icon: CheckCircle2,
              },
              {
                id: "paused",
                label: "Paused",
                count: downloads.filter((d) => d.state === "paused").length,
                icon: Pause,
              },
              {
                id: "queued",
                label: "Queued",
                count: downloads.filter((d) => d.state === "queued").length,
                icon: Clock,
              },
            ].map((item) => {
              const Icon = item.icon;
              const isActive = statusFilter === item.id && categoryFilter === "all";
              return (
                <div key={item.id} className="relative w-full flex items-center justify-center">
                  {isActive && (
                    <span className="absolute left-0 w-0.5 h-4 rounded-r bg-[var(--accent)]" />
                  )}
                  <Tooltip content={`${item.label} (${item.count})`} side="right">
                    <button
                      onClick={() => {
                        setStatusFilter(item.id);
                        setCategoryFilter("all");
                      }}
                      aria-label={`${item.label} (${item.count})`}
                      className={`w-9 h-9 relative flex items-center justify-center rounded-lg transition-colors duration-150 cursor-pointer ${
                        isActive
                          ? "bg-[var(--accent)]/12 text-[var(--accent)]"
                          : "bg-transparent text-[var(--text-muted)] hover:bg-white/[0.04] hover:text-[var(--text-primary)]"
                      }`}
                    >
                      <Icon className="w-4 h-4 stroke-[1.75]" />
                    </button>
                  </Tooltip>
                </div>
              );
            })}

            <div className="w-6 h-px bg-[var(--border-dim)] my-2" />

            <div className="relative w-full flex items-center justify-center">
              <Tooltip content="Settings" side="right">
                <button
                  onClick={() => setActiveView("settings")}
                  aria-label="Settings"
                  className="w-9 h-9 flex items-center justify-center rounded-lg text-[var(--text-muted)] hover:bg-white/[0.04] hover:text-[var(--text-primary)] transition-colors duration-150 cursor-pointer"
                >
                  <Settings className="w-4 h-4 stroke-[1.75]" />
                </button>
              </Tooltip>
            </div>

            <div className="relative w-full flex items-center justify-center">
              <Tooltip content="About EDM" side="right">
                <button
                  onClick={() => setIsAboutOpen(true)}
                  aria-label="About EDM"
                  className="w-9 h-9 flex items-center justify-center rounded-lg text-[var(--text-muted)] hover:bg-white/[0.04] hover:text-[var(--text-primary)] transition-colors duration-150 cursor-pointer"
                >
                  <Info className="w-4 h-4 stroke-[1.75]" />
                </button>
              </Tooltip>
            </div>
          </div>
        </aside>

        {/* Center Main Canvas: Quiet Neutral Surface, Edge-to-Edge Rows */}
        <main className="flex-1 flex flex-col min-w-0 bg-[var(--bg-chassis)] overflow-hidden">
          {viewMode === "table" ? (
            /* Table View with edge-to-edge hairline rows & rounded selection */
            <div className="flex-1 flex flex-col min-h-0 overflow-y-auto overflow-x-auto px-4 pt-2 pb-4">
              <table className="w-full table-fixed text-left border-separate border-spacing-y-0 min-w-[720px]">
                <thead className="sticky top-0 bg-[var(--bg-chassis)] border-b border-[var(--border-dim)] text-xs font-medium text-[var(--text-muted)] z-10">
                  <tr>
                    <th className="py-2 px-2 w-10 text-center whitespace-nowrap">
                      <input
                        type="checkbox"
                        checked={selectedIds.length === filteredDownloads.length && filteredDownloads.length > 0}
                        onChange={() => {
                          if (selectedIds.length === filteredDownloads.length) {
                            setSelectedIds([]);
                            setSelectedDownload(null);
                          } else {
                            setSelectedIds(filteredDownloads.map((d) => d.id));
                            setSelectedDownload(filteredDownloads[0] || null);
                          }
                        }}
                        className="edm-checkbox transition-transform active:scale-90"
                      />
                    </th>
                    <th
                      onClick={() => handleSort("name")}
                      className="py-2 px-3 w-[36%] xl:w-[40%] min-w-[170px] whitespace-nowrap cursor-pointer select-none hover:text-[var(--text-primary)]"
                    >
                      <div className="flex items-center gap-1">
                        <span>File Name</span>
                        {sortField === "name" && (sortDirection === "asc" ? <ChevronUp className="w-3 h-3 text-[var(--accent)]" /> : <ChevronDown className="w-3 h-3 text-[var(--accent)]" />)}
                      </div>
                    </th>
                    <th
                      onClick={() => handleSort("size")}
                      className="py-2 px-2 w-20 sm:w-24 text-center whitespace-nowrap cursor-pointer select-none hover:text-[var(--text-primary)]"
                    >
                      <div className="flex items-center justify-center gap-1">
                        <span>Size</span>
                        {sortField === "size" && (sortDirection === "asc" ? <ChevronUp className="w-3 h-3 text-[var(--accent)]" /> : <ChevronDown className="w-3 h-3 text-[var(--accent)]" />)}
                      </div>
                    </th>
                    <th
                      onClick={() => handleSort("progress")}
                      className="py-2 px-3 w-[26%] md:w-[24%] xl:w-[22%] min-w-[180px] whitespace-nowrap cursor-pointer select-none hover:text-[var(--text-primary)]"
                    >
                      <div className="flex items-center gap-1">
                        <span>Progress</span>
                        {sortField === "progress" && (sortDirection === "asc" ? <ChevronUp className="w-3 h-3 text-[var(--accent)]" /> : <ChevronDown className="w-3 h-3 text-[var(--accent)]" />)}
                      </div>
                    </th>
                    <th
                      onClick={() => handleSort("speed")}
                      className="py-2 px-2 w-24 md:w-28 text-center whitespace-nowrap cursor-pointer select-none hover:text-[var(--text-primary)]"
                    >
                      <div className="flex items-center justify-center gap-1">
                        <span>Speed</span>
                        {sortField === "speed" && (sortDirection === "asc" ? <ChevronUp className="w-3 h-3 text-[var(--accent)]" /> : <ChevronDown className="w-3 h-3 text-[var(--accent)]" />)}
                      </div>
                    </th>
                    <th className="py-2 px-3 pr-4 w-44 min-w-[176px] whitespace-nowrap text-center">
                      <span className="select-none">Actions</span>
                    </th>
                  </tr>
                </thead>
                <tbody className="text-xs">
                  {filteredDownloads.map((item) => {
                    const isSelected = selectedIds.includes(item.id);
                    return (
                      <tr
                        key={item.id}
                        onClick={() => selectRow(item)}
                        onDoubleClick={() => {
                          if (item.state === "completed" || item.progress >= 100) {
                            tauriOpenFile(item.id);
                          } else {
                            setIsInspectorOpen(true);
                          }
                        }}
                        onContextMenu={(e) => handleContextMenu(e, item)}
                        className={`group cursor-pointer table-row-3d ${
                          isSelected ? "table-row-selected-3d text-[var(--text-primary)]" : ""
                        }`}
                      >
                        <td className="py-2.5 px-2 text-center whitespace-nowrap" onClick={(e) => toggleSelect(item.id, e)}>
                          <input
                            type="checkbox"
                            checked={isSelected}
                            onChange={() => {}}
                            className="edm-checkbox transition-transform active:scale-90"
                          />
                        </td>
                        <td className="py-2.5 px-3 truncate">
                          <div className="flex items-center gap-2 truncate">
                            <div className="p-1.5 rounded-lg icon-box-3d shrink-0">
                              {getCategoryIcon(item.category)}
                            </div>
                            <div className="truncate min-w-0">
                              <div className="font-medium text-[var(--text-primary)] truncate" title={item.name}>
                                {item.name}
                              </div>
                              <div className="text-[10px] text-[var(--text-muted)] truncate flex items-center gap-1">
                                <span className="truncate">{item.domain}</span>
                                <span>•</span>
                                <span className="shrink-0">
                                  {item.state === "downloading" && item.eta ? `ETA: ${item.eta}` : item.dateAdded}
                                </span>
                              </div>
                            </div>
                          </div>
                        </td>
                        <td className="py-3 px-2 tabular-nums text-xs text-[var(--text-secondary)] text-center whitespace-nowrap">
                          {formatBytes(item.sizeTotal)}
                        </td>
                        <td className="py-3 px-3 min-w-0 overflow-hidden">
                          <div className="space-y-1.5 min-w-0">
                            <div className="flex items-center justify-between text-xs tabular-nums min-w-0 gap-2">
                              <div className="flex items-center gap-1.5 shrink-0">
                                <span
                                  className={`w-1.5 h-1.5 rounded-full ${
                                    item.state === "completed" || item.progress >= 100
                                      ? "bg-emerald-400"
                                      : item.state === "downloading"
                                        ? "bg-[var(--accent)]"
                                        : item.state === "paused"
                                          ? "bg-amber-400"
                                          : item.state === "failed"
                                            ? "bg-rose-400"
                                            : "bg-zinc-500"
                                  }`}
                                />
                                <span className="font-medium text-[var(--text-primary)]">
                                  {item.state === "completed" || item.progress >= 100 ? 100 : item.progress}%
                                </span>
                              </div>
                              <span className="text-[var(--text-secondary)] truncate text-right">
                                {formatBytes(item.state === "completed" || item.progress >= 100 ? (item.sizeTotal || item.sizeDone) : item.sizeDone)} / {formatBytes(item.sizeTotal || item.sizeDone)}
                              </span>
                            </div>
                            {/* Semantic Progress Bar (Completed=Emerald, Downloading=Accent+Sweep, Paused=Amber+Sweep, Failed=Rose, Queued=Zinc Sweep) */}
                            <div className="w-full h-1.5 rounded-full overflow-hidden track-3d relative bg-white/[0.06]">
                              {item.state === "queued" && item.progress === 0 ? (
                                <div className="w-1/4 h-full rounded-full bg-gradient-to-r from-transparent via-zinc-400/70 to-transparent edm-queue-sweep" />
                              ) : (
                                <>
                                  <div
                                    className={`h-full rounded-full transition-[width,background-color] duration-200 ease-linear ${
                                      item.state === "completed" || item.progress >= 100
                                        ? "bg-emerald-500"
                                        : item.state === "downloading"
                                          ? "bg-[var(--accent)]"
                                          : item.state === "paused"
                                            ? "bg-amber-500/85"
                                            : item.state === "failed"
                                              ? "bg-rose-500/85"
                                              : "bg-zinc-500/70"
                                    }`}
                                    style={{
                                      width: `${item.state === "completed" || item.progress >= 100 ? 100 : Math.min(100, Math.max(0, item.progress))}%`,
                                    }}
                                  />
                                  {(item.state === "downloading" || item.state === "paused" || item.state === "queued") && item.progress < 100 && (
                                    <div
                                      className="absolute inset-y-0 right-0 overflow-hidden pointer-events-none"
                                      style={{ left: `${Math.min(100, Math.max(0, item.progress))}%` }}
                                    >
                                      <div
                                        className={`w-1/3 h-full rounded-full bg-gradient-to-r from-transparent ${
                                          item.state === "paused"
                                            ? "via-amber-300/65"
                                            : item.state === "queued"
                                              ? "via-zinc-300/70"
                                              : "via-[var(--accent)]/60"
                                        } to-transparent edm-queue-sweep`}
                                      />
                                    </div>
                                  )}
                                </>
                              )}
                            </div>
                          </div>
                        </td>
                        <td className="py-3 px-2 tabular-nums font-semibold text-[var(--accent)] text-center whitespace-nowrap text-xs">
                          <div className="inline-flex items-center justify-center gap-1">
                            {item.state === "downloading" && (
                              <Zap className="w-3.5 h-3.5 fill-current shrink-0" />
                            )}
                            <span>{item.speed}</span>
                          </div>
                        </td>
                        <td className="py-3 px-3 pr-4 w-44 min-w-[176px] text-center whitespace-nowrap">
                          <div className="flex items-center justify-center gap-2">
                            {item.state === "completed" ? (
                              <Tooltip content="Open File" side="top">
                                <button
                                  onClick={(e) => {
                                    e.stopPropagation();
                                    tauriOpenFile(item.id);
                                  }}
                                  className="w-8 h-8 rounded-full bg-emerald-500/15 text-emerald-400 hover:bg-emerald-500/25 flex items-center justify-center cursor-pointer transition-colors"
                                  aria-label={`Open ${item.name}`}
                                >
                                  <CheckCircle2 className="w-4 h-4 stroke-[1.75]" />
                                </button>
                              </Tooltip>
                            ) : item.state === "downloading" ? (
                              <Tooltip content="Pause Download" side="top">
                                <button
                                  onClick={(e) => toggleDownloadState(item.id, e)}
                                  className="w-8 h-8 rounded-full bg-[var(--accent)]/15 text-[var(--accent)] hover:bg-[var(--accent)]/25 flex items-center justify-center cursor-pointer transition-colors"
                                  aria-label={`Pause ${item.name}`}
                                >
                                  <Pause className="w-4 h-4 stroke-[1.75]" />
                                </button>
                              </Tooltip>
                            ) : item.state === "paused" ? (
                              <Tooltip content="Resume Download" side="top">
                                <button
                                  onClick={(e) => toggleDownloadState(item.id, e)}
                                  className="w-8 h-8 rounded-full bg-amber-500/15 text-amber-400 hover:bg-amber-500/25 flex items-center justify-center cursor-pointer transition-colors"
                                  aria-label={`Resume ${item.name}`}
                                >
                                  <Play className="w-4 h-4 stroke-[1.75]" />
                                </button>
                              </Tooltip>
                            ) : item.state === "failed" ? (
                              <Tooltip content="Retry / Refresh" side="top">
                                <button
                                  onClick={(e) => {
                                    if (item.error_msg?.toLowerCase().includes("expire") || item.error_msg?.includes("401") || item.error_msg?.includes("403") || item.error_msg?.includes("410")) {
                                      e.stopPropagation();
                                      setRefreshModalTask(item);
                                    } else {
                                      toggleDownloadState(item.id, e);
                                    }
                                  }}
                                  className="w-8 h-8 rounded-full bg-rose-500/15 text-rose-400 hover:bg-rose-500/25 flex items-center justify-center cursor-pointer transition-colors"
                                  aria-label={`Retry ${item.name}`}
                                >
                                  <RotateCcw className="w-4 h-4 stroke-[1.75]" />
                                </button>
                              </Tooltip>
                            ) : (
                              <Tooltip content="Start Download" side="top">
                                <button
                                  onClick={(e) => toggleDownloadState(item.id, e)}
                                  className="w-8 h-8 rounded-full bg-[var(--accent)]/15 text-[var(--accent)] hover:bg-[var(--accent)]/25 flex items-center justify-center cursor-pointer transition-colors"
                                  aria-label={`Start ${item.name}`}
                                >
                                  <Play className="w-4 h-4 stroke-[1.75]" />
                                </button>
                              </Tooltip>
                            )}
                            <div className="inline-flex items-center p-0.5 rounded-full bg-white/[0.03] border border-[var(--border-dim)]">
                              <Tooltip content="Reveal in Explorer" side="top">
                                <button
                                  onClick={(e) => {
                                    e.stopPropagation();
                                    tauriOpenFolder(item.id);
                                  }}
                                  className="w-7 h-7 rounded-full text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-white/[0.07] flex items-center justify-center cursor-pointer transition-colors"
                                  aria-label={`Reveal ${item.name} in Explorer`}
                                >
                                  <FolderOpen className="w-3.5 h-3.5 stroke-[1.75]" />
                                </button>
                              </Tooltip>
                              <Tooltip content="Download Status" side="top">
                                <button
                                  onClick={(e) => {
                                    e.stopPropagation();
                                    setSelectedDownload(item);
                                    if (!selectedIds.includes(item.id)) {
                                      setSelectedIds([item.id]);
                                    }
                                    setIsInspectorOpen(true);
                                  }}
                                  className="w-7 h-7 rounded-full text-[var(--text-secondary)] hover:text-[var(--accent)] hover:bg-white/[0.07] flex items-center justify-center cursor-pointer transition-colors"
                                  aria-label={`Download Status for ${item.name}`}
                                >
                                  <Layers className="w-3.5 h-3.5 stroke-[1.75]" />
                                </button>
                              </Tooltip>
                              <Tooltip content="Delete Task" side="top" align="end">
                                <button
                                  onClick={(e) => {
                                    e.stopPropagation();
                                    handleRequestDelete(item.id, item.name);
                                  }}
                                  className="w-7 h-7 rounded-full text-[var(--text-secondary)] hover:text-rose-400 hover:bg-rose-500/12 flex items-center justify-center cursor-pointer transition-colors"
                                  aria-label={`Delete ${item.name}`}
                                >
                                  <Trash2 className="w-3.5 h-3.5 stroke-[1.75]" />
                                </button>
                              </Tooltip>
                            </div>
                          </div>
                        </td>
                      </tr>
                    );
                  })}
                  {filteredDownloads.length === 0 && (
                    <tr>
                      <td colSpan={7} className="py-24 text-center">
                        <div className="flex flex-col items-center justify-center space-y-3">
                          <div className="w-12 h-12 rounded-2xl bg-[var(--bg-card)] border border-[var(--border-subtle)] flex items-center justify-center text-[var(--accent)] shadow-sm">
                            <Download className="w-6 h-6 stroke-[1.75]" />
                          </div>
                          <div className="space-y-1">
                            <p className="text-sm font-semibold text-[var(--text-primary)]">No active downloads</p>
                            <p className="text-xs text-[var(--text-muted)] max-w-sm">
                              Click "+ Add URL" or paste a download link to begin accelerated transfer.
                            </p>
                          </div>
                          <button
                            onClick={() => setIsAddModalOpen(true)}
                            className="mt-2 btn-primary-3d px-4 py-2 rounded-lg text-xs font-semibold inline-flex items-center gap-1.5 cursor-pointer"
                          >
                            <Plus className="w-4 h-4" />
                            <span>Add Download URL</span>
                          </button>
                        </div>
                      </td>
                    </tr>
                  )}
                </tbody>
              </table>
            </div>
          ) : (
            /* Grid Card View (Dense, Crafted, No Void Space) */
            <div className="flex-1 p-4 overflow-y-auto space-y-4">
              <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-3.5 auto-rows-max">
                {filteredDownloads.map((item) => (
                  <div
                    key={item.id}
                    onClick={() => selectRow(item)}
                    onDoubleClick={() => {
                      if (item.state === "completed" || item.progress >= 100) {
                        tauriOpenFile(item.id);
                      } else {
                        setIsInspectorOpen(true);
                      }
                    }}
                    onContextMenu={(e) => handleContextMenu(e, item)}
                    className={`card-3d p-3.5 rounded-xl flex flex-col gap-2.5 cursor-pointer select-none transition-all ${
                      selectedIds.includes(item.id) ? "border-[var(--accent)] ring-1 ring-[var(--accent-glow)]" : ""
                    }`}
                  >
                    {/* Top Row: File Identity & Status */}
                    <div className="flex items-start justify-between gap-2.5">
                      <div className="flex items-center gap-2.5 min-w-0">
                        <div className="p-2 rounded-lg icon-box-3d shrink-0">
                          {getCategoryIcon(item.category)}
                        </div>
                        <div className="min-w-0">
                          <div className="font-semibold text-xs text-[var(--text-primary)] truncate" title={item.name}>
                            {item.name}
                          </div>
                          <div className="text-[11px] text-[var(--text-muted)] truncate flex items-center gap-1.5">
                            <span>{item.domain}</span>
                            <span>•</span>
                            <span>{item.dateAdded}</span>
                          </div>
                        </div>
                      </div>
                      <div className="shrink-0">{getStatusBadge(item.state, item.error_msg)}</div>
                    </div>

                    {/* Middle Info Strip: Technical Metadata */}
                    <div className="flex items-center justify-between px-2.5 py-1.5 rounded-lg input-3d text-[11px] font-mono tabular-nums text-[var(--text-secondary)]">
                      <div className="flex items-center gap-1.5">
                        <Cpu className="w-3 h-3 text-[var(--accent)]" />
                        <span>{item.connections > 0 ? `${item.connections} Streams` : "Idle"}</span>
                      </div>
                      <div className="text-[var(--text-muted)]">•</div>
                      <div>{formatBytes(item.sizeTotal)}</div>
                      <div className="text-[var(--text-muted)]">•</div>
                      <div
                        className={
                          item.state === "completed" || item.progress >= 100
                            ? "text-emerald-400 font-semibold"
                            : item.state === "downloading"
                            ? "text-cyan-400 font-semibold"
                            : item.state === "paused"
                            ? "text-amber-400 font-semibold"
                            : item.state === "failed"
                            ? "text-rose-400 font-semibold"
                            : "text-[var(--text-muted)]"
                        }
                      >
                        {item.state === "completed" || item.progress >= 100
                          ? "Verified"
                          : item.state === "downloading"
                          ? (item.connections > 1 ? "HTTP 206" : "HTTP 200")
                          : item.state === "paused"
                          ? "Paused"
                          : item.state === "failed"
                          ? "Failed"
                          : "Queued"}
                      </div>
                    </div>

                    {/* Progress & Micro-Segment Track */}
                    <div className="space-y-1">
                      <div className="flex justify-between text-xs font-mono tabular-nums">
                        <span className="font-semibold text-[var(--accent)]">{item.state === "completed" || item.progress >= 100 ? 100 : item.progress}%</span>
                        <span className="text-[var(--text-muted)]">
                          {formatBytes(item.state === "completed" || item.progress >= 100 ? (item.sizeTotal || item.sizeDone) : item.sizeDone)} / {formatBytes(item.sizeTotal || item.sizeDone)}
                        </span>
                      </div>
                      {/* Plain Seamless Progress Bar (100% smooth, zero vertical lines) */}
                      <div className="w-full h-2 rounded bg-[var(--bg-chassis)] overflow-hidden border border-[var(--border-subtle)] relative">
                        <div
                          className="h-full rounded bg-[var(--accent)] transition-[width] duration-200 ease-linear"
                          style={{
                            width: `${item.state === "completed" || item.progress >= 100 ? 100 : Math.min(100, Math.max(0, item.progress))}%`,
                            opacity: item.state === "paused" ? 0.6 : 1,
                          }}
                        />
                        {item.state === "downloading" && (item.eta?.includes("Merging") || item.eta?.includes("Fetching") || !item.sizeTotal || item.sizeTotal === 0) ? (
                          <div className="absolute inset-0 bg-gradient-to-r from-transparent via-[var(--accent)]/50 to-transparent pointer-events-none animate-[shimmer_1.5s_infinite]" />
                        ) : item.state === "downloading" ? (
                          <div className="absolute inset-0 bg-white/10 pointer-events-none animate-[pulse_1.5s_infinite]" />
                        ) : null}
                      </div>
                    </div>

                    {/* Metrics & Inline Actions */}
                    <div className="flex items-center justify-between pt-2 border-t border-[var(--border-subtle)] text-xs">
                      <div className="font-mono tabular-nums">
                        <span className="font-semibold text-[var(--accent)]">{item.speed}</span>
                        <span className="text-[var(--text-muted)] ml-2 text-[11px]">ETA: {item.eta}</span>
                      </div>
                      <div className="flex items-center gap-1.5">
                        {item.state === "completed" ? (
                          <button
                            onClick={(e) => {
                              e.stopPropagation();
                              tauriOpenFile(item.id);
                            }}
                            className="btn-3d p-1.5 rounded-md text-emerald-400 hover:text-emerald-300 hover:bg-emerald-500/10 cursor-pointer transition-all"
                            title="Download Complete • Click to Open File"
                          >
                            <CheckCircle2 className="w-4 h-4 text-emerald-400" />
                          </button>
                        ) : item.state === "downloading" ? (
                          <button
                            onClick={(e) => toggleDownloadState(item.id, e)}
                            className="btn-3d p-1.5 rounded-md text-amber-400 hover:text-amber-300 hover:bg-amber-500/10 cursor-pointer transition-all"
                            title="Pause Download"
                          >
                            <Pause className="w-4 h-4" />
                          </button>
                        ) : item.state === "paused" ? (
                          <button
                            onClick={(e) => toggleDownloadState(item.id, e)}
                            className="btn-3d p-1.5 rounded-md text-emerald-400 hover:text-emerald-300 hover:bg-emerald-500/10 cursor-pointer transition-all"
                            title="Resume Download"
                          >
                            <Play className="w-4 h-4 text-emerald-400" />
                          </button>
                        ) : item.state === "failed" ? (
                          item.error_msg?.toLowerCase().includes("expire") || item.error_msg?.includes("401") || item.error_msg?.includes("403") || item.error_msg?.includes("410") ? (
                            <button
                              onClick={(e) => {
                                e.stopPropagation();
                                setRefreshModalTask(item);
                              }}
                              className="btn-3d p-1.5 rounded-md text-amber-400 hover:text-amber-300 hover:bg-amber-500/10 cursor-pointer transition-all"
                              title="Refresh Expired Download Link"
                            >
                              <RotateCcw className="w-4 h-4 text-amber-400" />
                            </button>
                          ) : (
                            <button
                              onClick={(e) => toggleDownloadState(item.id, e)}
                              className="btn-3d p-1.5 rounded-md text-rose-400 hover:text-rose-300 hover:bg-rose-500/10 cursor-pointer transition-all"
                              title="Retry Download"
                            >
                              <RotateCcw className="w-4 h-4 text-rose-400" />
                            </button>
                          )
                        ) : (
                          <button
                            onClick={(e) => toggleDownloadState(item.id, e)}
                            className="btn-3d p-1.5 rounded-md text-[var(--accent)] hover:brightness-110 cursor-pointer transition-all"
                            title="Start Download"
                          >
                            <Play className="w-4 h-4" />
                          </button>
                        )}
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            setSelectedDownload(item);
                            if (!selectedIds.includes(item.id)) {
                              setSelectedIds([item.id]);
                            }
                            setIsInspectorOpen(true);
                          }}
                          className="btn-3d p-1.5 rounded-md text-[var(--accent)] hover:brightness-110 cursor-pointer"
                          title="Download Status"
                        >
                          <Layers className="w-4 h-4" />
                        </button>
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            tauriOpenFolder(item.id);
                          }}
                          className="btn-3d p-1.5 rounded-md text-[var(--text-secondary)] hover:text-[var(--text-primary)] cursor-pointer"
                          title="Reveal in Explorer"
                        >
                          <FolderOpen className="w-4 h-4" />
                        </button>
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            handleRequestDelete(item.id, item.name);
                          }}
                          className="btn-3d p-1.5 rounded-md text-[var(--text-muted)] hover:text-rose-400 hover:bg-rose-500/10 cursor-pointer transition-all"
                          title="Delete Task"
                        >
                          <Trash2 className="w-4 h-4" />
                        </button>
                      </div>
                    </div>
                  </div>
                ))}
                {filteredDownloads.length === 0 && (
                  <div className="col-span-full py-24 text-center flex flex-col items-center justify-center space-y-3">
                    <div className="w-12 h-12 rounded-2xl bg-[var(--bg-card)] border border-[var(--border-subtle)] flex items-center justify-center text-[var(--accent)] shadow-sm">
                      <Download className="w-6 h-6 stroke-[1.75]" />
                    </div>
                    <div className="space-y-1">
                      <p className="text-sm font-semibold text-[var(--text-primary)]">No active downloads</p>
                      <p className="text-xs text-[var(--text-muted)] max-w-sm">
                        Click "+ Add URL" or paste a download link to begin accelerated transfer.
                      </p>
                    </div>
                    <button
                      onClick={() => setIsAddModalOpen(true)}
                      className="mt-2 btn-primary-3d px-4 py-2 rounded-lg text-xs font-semibold inline-flex items-center gap-1.5 cursor-pointer"
                    >
                      <Plus className="w-4 h-4" />
                      <span>Add Download URL</span>
                    </button>
                  </div>
                )}
              </div>
            </div>
          )}
        </main>
      </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* ─────────────────────────────────────────────────────────────
          3. PERSISTENT INTEGRATED BOTTOM STATUS BAR
      ────────────────────────────────────────────────────────────── */}
      <StatusFooter
        activeView={activeView}
        onToggleView={() => {
          if (activeView === "settings") {
            setActiveView("downloads");
            window.location.hash = "";
          } else {
            setActiveView("settings");
          }
        }}
        onOpenScheduler={() => setIsSchedulerOpen(true)}
        speedLimit={speedLimit}
        onSpeedLimitChange={handleSpeedLimitChange}
        selectedDownload={activeSelectedDownload}
        totalSpeed={totalSpeedDisplay}
        totalUploadSpeed={totalUploadSpeedDisplay}
        activeThreads={totalActiveThreads}
        totalSessionBytes={totalSessionBytes}
      />

      {/* ─────────────────────────────────────────────────────────────
          4. MODAL DIALOGS & OVERLAYS (Modular Components)
      ────────────────────────────────────────────────────────────── */}
      <DetailsInspectorDrawer
        isOpen={isInspectorOpen}
        download={activeSelectedDownload}
        onClose={() => setIsInspectorOpen(false)}
        settings={settings}
        onToggleState={toggleDownloadState}
        onOpenFile={tauriOpenFile}
      />

      <AddDownloadModal
        isOpen={isAddModalOpen}
        onClose={() => {
          setIsAddModalOpen(false);
          setIsAddSchedulerMode(false);
          setPendingCapture(null);
        }}
        initialQueueMode={isAddSchedulerMode}
        initialUrl={pendingCapture?.url}
        initialFilename={pendingCapture?.filename}
        initialSize={pendingCapture?.size}
        onAdd={(url, filename, category, conns, saveDir, selectedFiles, scheduleQueueId, queueOnly) => {
          if ((scheduleQueueId || queueOnly) && !isTauri()) {
            const parsedName = filename || url.split("/").pop()?.split("?")[0] || "scheduled-download";
            let domain = "scheduled";
            try {
              domain = new URL(url).hostname;
            } catch {}
            setDownloads((prev) => [
              {
                id: `sched-${Date.now()}`,
                name: parsedName,
                category: (category as DownloadItem["category"]) || "Other",
                sizeTotal: 0,
                sizeDone: 0,
                speed: "0 B/s",
                eta: "Queued",
                connections: 0,
                state: "queued",
                progress: 0,
                url,
                domain,
                createdAt: Date.now(),
                dateAdded: "Just now",
              },
              ...prev,
            ]);
            return;
          }
          handleAddDownload(url, filename, category, conns, saveDir, selectedFiles, queueOnly || Boolean(scheduleQueueId));
        }}
        onOpenBatchImport={() => setIsBatchImportOpen(true)}
        settings={settings}
      />

      <BatchImportModal
        isOpen={isBatchImportOpen}
        onClose={() => setIsBatchImportOpen(false)}
        onQueueSelected={async (links: ImportedLink[]) => {
          for (const link of links) {
            await handleAddDownload(
              link.url,
              link.filename_hint ?? undefined,
              link.category_hint ?? undefined
            );
          }
        }}
      />

      <AnimatePresence>
        {isSchedulerOpen && (
          <motion.div
            variants={backdropVariants}
            initial="initial"
            animate="animate"
            exit="exit"
            className="fixed inset-0 bg-black/75 backdrop-blur-sm flex items-center justify-center p-4 z-50"
            onClick={() => setIsSchedulerOpen(false)}
          >
            <motion.div
              variants={modalVariants}
              initial="initial"
              animate="animate"
              exit="exit"
              role="dialog"
              aria-modal="true"
              aria-label="Queue Scheduler"
              className="w-full max-w-2xl max-h-[85vh] rounded-xl bg-[var(--bg-panel)] border border-[var(--border-subtle)] shadow-[0_24px_48px_-12px_rgba(0,0,0,0.7)] flex flex-col overflow-hidden"
              onClick={(e) => e.stopPropagation()}
            >
              <div className="px-4 py-3 border-b border-[var(--border-dim)] flex items-center justify-between shrink-0">
                <span className="font-semibold tracking-tight text-xs sm:text-sm text-[var(--text-primary)] flex items-center gap-2">
                  <CalendarClock className="w-4 h-4 stroke-[1.75] text-[var(--accent)]" />
                  Queue Scheduler
                </span>
                <button
                  onClick={() => setIsSchedulerOpen(false)}
                  aria-label="Close Queue Scheduler"
                  className="w-7 h-7 rounded-lg flex items-center justify-center text-[var(--text-muted)] hover:text-[var(--text-primary)] hover:bg-white/[0.06] transition-colors cursor-pointer"
                >
                  <X className="w-4 h-4 stroke-[1.75]" />
                </button>
              </div>
              <div className="p-4 overflow-y-auto">
                <QueueSchedulerPanel />
              </div>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>

      <SetupWizard
        isOpen={isWizardOpen}
        onClose={() => setIsWizardOpen(false)}
        settings={settings}
        onSaveSettings={handleUpdateSettings}
      />

      <RefreshLinkModal
        isOpen={Boolean(refreshModalTask)}
        task={refreshModalTask}
        onClose={() => setRefreshModalTask(null)}
        onRefreshed={(taskId, newUrl) => {
          setDownloads((prev) =>
            prev.map((d) =>
              d.id === taskId
                ? { ...d, url: newUrl, state: "downloading", speed: "Connecting..." }
                : d
            )
          );
        }}
      />

      <PowerCountdownModal />

      {editRequestTask && (
        <EditRequestModal
          isOpen={Boolean(editRequestTask)}
          taskId={editRequestTask.id}
          initialUrl={editRequestTask.url}
          onClose={() => setEditRequestTask(null)}
          onSaved={(override) => {
            if (override.url) {
              setDownloads((prev) =>
                prev.map((d) =>
                  d.id === editRequestTask.id ? { ...d, url: override.url! } : d
                )
              );
            }
          }}
        />
      )}

      {/* ─────────────────────────────────────────────────────────────
          DELETE CONFIRMATION MODAL (Glassmorphic)
      ────────────────────────────────────────────────────────────── */}
      <AnimatePresence>
        {deleteModalTarget && (
          <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
            {/* Backdrop */}
            <motion.div
              variants={backdropVariants}
              initial="initial"
              animate="animate"
              exit="exit"
              onClick={() => {
                setDeleteModalTarget(null);
                setDeleteModalDontAsk(false);
              }}
              className="fixed inset-0 bg-black/60 backdrop-blur-sm"
            />

            {/* Modal Window */}
            <motion.div
              variants={modalVariants}
              initial="initial"
              animate="animate"
              exit="exit"
              className="relative w-full max-w-md rounded-2xl panel-3d shadow-2xl overflow-hidden flex flex-col z-10 border border-[var(--border-subtle)] bg-[var(--bg-card)]/95 backdrop-blur-xl"
            >
              {/* Header */}
              <div className="px-5 py-4 border-b border-[var(--border-subtle)] flex items-center justify-between bg-[var(--bg-panel)]">
                <div className="flex items-center gap-2.5">
                  <div className="p-2 rounded-xl bg-rose-500/10 text-rose-400">
                    <Trash2 className="w-4 h-4" />
                  </div>
                  <div>
                    <h3 className="text-sm font-semibold tracking-tight text-[var(--text-primary)]">
                      Remove Download
                    </h3>
                    <p className="text-xs text-[var(--text-muted)] truncate max-w-[280px]">
                      Are you sure you want to remove <span className="text-[var(--text-primary)] font-medium">{deleteModalTarget.name}</span>?
                    </p>
                  </div>
                </div>
                <button
                  onClick={() => {
                    setDeleteModalTarget(null);
                    setDeleteModalDontAsk(false);
                  }}
                  className="p-1 rounded-lg text-[var(--text-muted)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
                >
                  <X className="w-4 h-4" />
                </button>
              </div>

              {/* Body */}
              <div className="p-5 space-y-4">
                <div className="space-y-2.5">
                  {/* Choice 1: Remove from List Only */}
                  <button
                    type="button"
                    onClick={() => handleConfirmDelete(false)}
                    className="w-full p-3.5 rounded-xl border border-[var(--border-subtle)] hover:border-[var(--accent)] hover:bg-[var(--bg-active)] flex items-start gap-3 text-left transition-all cursor-pointer group"
                  >
                    <div className="p-2 rounded-lg bg-[var(--bg-chassis)] group-hover:bg-[var(--accent)]/10 text-[var(--text-secondary)] group-hover:text-[var(--accent)] mt-0.5 shrink-0 transition-colors">
                      <LayoutList className="w-4 h-4" />
                    </div>
                    <div className="flex-1 min-w-0">
                      <span className="text-xs font-semibold text-[var(--text-primary)] block">
                        Remove from List Only
                      </span>
                      <span className="text-[11px] text-[var(--text-muted)] block mt-0.5">
                        Keeps file on disk
                      </span>
                    </div>
                  </button>

                  {/* Choice 2: Delete File & Remove */}
                  <button
                    type="button"
                    onClick={() => handleConfirmDelete(true)}
                    className="w-full p-3.5 rounded-xl border border-rose-500/20 hover:border-rose-500/60 hover:bg-rose-500/5 flex items-start gap-3 text-left transition-all cursor-pointer group"
                  >
                    <div className="p-2 rounded-lg bg-rose-500/10 text-rose-400 group-hover:bg-rose-500/20 mt-0.5 shrink-0 transition-colors">
                      <Trash2 className="w-4 h-4" />
                    </div>
                    <div className="flex-1 min-w-0">
                      <span className="text-xs font-semibold text-rose-300 block">
                        Delete File &amp; Remove from List
                      </span>
                      <span className="text-[11px] text-[var(--text-muted)] block mt-0.5">
                        Safely moves to Recycle Bin (permanent delete fallback)
                      </span>
                    </div>
                  </button>
                </div>

                {/* Don't ask again checkbox */}
                <div className="pt-2 border-t border-[var(--border-subtle)]">
                  <label className="flex items-center gap-2.5 cursor-pointer select-none">
                    <input
                      type="checkbox"
                      checked={deleteModalDontAsk}
                      onChange={(e) => setDeleteModalDontAsk(e.target.checked)}
                      className="edm-checkbox"
                    />
                    <span className="text-xs text-[var(--text-secondary)]">
                      Don't ask again (You can change this anytime in Settings)
                    </span>
                  </label>
                </div>
              </div>

              {/* Footer */}
              <div className="px-5 py-3 border-t border-[var(--border-subtle)] bg-[var(--bg-panel)] flex justify-end">
                <button
                  type="button"
                  onClick={() => {
                    setDeleteModalTarget(null);
                    setDeleteModalDontAsk(false);
                  }}
                  className="btn-3d px-4 py-2 rounded-xl text-xs font-semibold text-[var(--text-secondary)] hover:text-[var(--text-primary)] cursor-pointer"
                >
                  Cancel
                </button>
              </div>
            </motion.div>
          </div>
        )}
      </AnimatePresence>

      {/* Floating About EDM Modal */}
      <AnimatePresence>
        {isAboutOpen && (
          <motion.div
            variants={backdropVariants}
            initial="initial"
            animate="animate"
            exit="exit"
            onClick={() => setIsAboutOpen(false)}
            className="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4"
          >
            <motion.div
              variants={modalVariants}
              initial="initial"
              animate="animate"
              exit="exit"
              onClick={(e) => e.stopPropagation()}
              role="dialog"
              aria-modal="true"
              aria-label="About Evo Download Manager"
              className="w-full max-w-sm rounded-xl bg-[var(--bg-panel)] border border-[var(--border-subtle)] shadow-[0_24px_48px_-12px_rgba(0,0,0,0.7)] p-5 space-y-4 select-none"
            >
              <div className="flex items-start justify-between gap-3">
                <div className="flex items-center gap-3">
                  <img
                    src="/logo.png"
                    alt="EDM Logo"
                    className="w-10 h-10 object-contain shrink-0 pointer-events-none"
                  />
                  <div>
                    <div className="flex items-center gap-2">
                      <h2 className="text-sm font-semibold tracking-tight text-[var(--text-primary)]">
                        Evo Download Manager
                      </h2>
                      <span className="px-1.5 py-0.5 rounded bg-[var(--accent)]/15 border border-[var(--accent)]/30 text-[11px] font-medium tabular-nums text-[var(--accent)]">
                        v{APP_VERSION}
                      </span>
                    </div>
                    <p className="text-xs text-[var(--text-secondary)] mt-0.5">
                      Built by <span className="font-medium text-[var(--text-primary)]">bytebldr</span>
                    </p>
                  </div>
                </div>
                <button
                  onClick={() => setIsAboutOpen(false)}
                  aria-label="Close About dialog"
                  className="w-7 h-7 rounded-lg flex items-center justify-center text-[var(--text-muted)] hover:text-[var(--text-primary)] hover:bg-white/[0.06] transition-colors cursor-pointer"
                >
                  <X className="w-4 h-4 stroke-[1.75]" />
                </button>
              </div>

              <p className="text-xs leading-relaxed text-[var(--text-secondary)]">
                Evo Download Manager is an open-source download manager built with a focus on speed, reliability, and simplicity. Our goal is to make downloading files faster and more seamless. Contributions, feedback, and suggestions are always welcome. If you'd like to get involved, feel free to share your ideas or submit a pull request.
              </p>

              <div className="pt-2 border-t border-[var(--border-dim)] flex items-center justify-between gap-2 text-xs">
                <span className="text-[var(--text-muted)]">GPL-3.0 License</span>
                <Tooltip content="GitHub Repository" side="top" align="end">
                  <a
                    href="https://github.com/bytebldr/evo-download-manager"
                    target="_blank"
                    rel="noreferrer"
                    aria-label="Open EDM GitHub Repository"
                    className="w-8 h-8 rounded-lg bg-white/[0.04] border border-[var(--border-dim)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-white/[0.08] flex items-center justify-center transition-colors"
                  >
                    <svg viewBox="0 0 24 24" className="w-4 h-4 fill-current" aria-hidden="true">
                      <path d="M12 0C5.37 0 0 5.37 0 12c0 5.31 3.435 9.795 8.205 11.385.6.105.825-.255.825-.57 0-.285-.015-1.23-.015-2.235-3.015.555-3.795-.735-4.035-1.41-.135-.345-.72-1.41-1.23-1.695-.42-.225-1.02-.78-.015-.795.945-.015 1.62.87 1.845 1.23 1.08 1.815 2.805 1.305 3.495.99.105-.78.42-1.305.765-1.605-2.67-.3-5.46-1.335-5.46-5.925 0-1.305.465-2.385 1.23-3.225-.12-.3-.54-1.53.12-3.18 0 0 1.005-.315 3.3 1.23.96-.27 1.98-.405 3-.405s2.04.135 3 .405c2.295-1.56 3.3-1.23 3.3-1.23.66 1.65.24 2.88.12 3.18.765.84 1.23 1.905 1.23 3.225 0 4.605-2.805 5.625-5.475 5.925.435.375.81 1.095.81 2.22 0 1.605-.015 2.895-.015 3.3 0 .315.225.69.825.57A12.02 12.02 0 0024 12c0-6.63-5.37-12-12-12z" />
                    </svg>
                  </a>
                </Tooltip>
              </div>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Modern Context Menu */}
      <DownloadContextMenu
        visible={contextMenu.visible}
        x={contextMenu.x}
        y={contextMenu.y}
        item={contextMenu.item}
        onClose={() => setContextMenu((p) => ({ ...p, visible: false }))}
        onOpenFile={tauriOpenFile}
        onPause={(id) => {
          tauriPauseDownload(id);
          setDownloads((prev) =>
            prev.map((d) => (d.id === id ? { ...d, state: "paused", speed: "0 B/s" } : d))
          );
        }}
        onResume={(id) => {
          tauriResumeDownload(id);
          setDownloads((prev) =>
            prev.map((d) => (d.id === id ? { ...d, state: "downloading" } : d))
          );
        }}
        onOpenFolder={tauriOpenFolder}
        onInspect={(item) => {
          setSelectedDownload(item);
          setIsInspectorOpen(true);
        }}
        onEditRequest={(item) => setEditRequestTask(item)}
        onDelete={handleRequestDelete}
      />

      {/* ─────────────────────────────────────────────────────────────
          FLOATING IN-APP UPDATE BANNER (Theme-Adaptive Flat Surface)
      ────────────────────────────────────────────────────────────── */}
      <AnimatePresence>
        {showUpdateBanner && updateInfo && (
          <motion.div
            initial={{ opacity: 0, y: 12, scale: 0.98 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: 12, scale: 0.98 }}
            transition={{ duration: 0.16, ease: "easeOut" }}
            role="alert"
            aria-label="Update notification"
            className="fixed z-50 bottom-11 left-1/2 -translate-x-1/2 flex items-center gap-3 px-3.5 py-2 rounded-lg bg-[var(--bg-elevated)]/95 backdrop-blur-md border border-[var(--border-subtle)] shadow-[0_8px_24px_rgba(0,0,0,0.24)] text-xs w-auto max-w-[90vw] select-none"
          >
            {/* Status Dot + Version & Release Notes */}
            <div className="flex items-center gap-2 min-w-0">
              <span
                className="w-2 h-2 rounded-full shrink-0"
                style={{ backgroundColor: "var(--accent-primary)" }}
              />
              <span className="font-semibold text-[var(--text-primary)] text-xs whitespace-nowrap tabular-nums">
                Update available: v{updateInfo.latest_version}
              </span>
              {updateInfo.html_url && updateInfo.html_url.startsWith("https://") && (
                <a
                  href={updateInfo.html_url}
                  target="_blank"
                  rel="noreferrer"
                  className="text-[11px] font-medium hover:underline underline-offset-2 whitespace-nowrap"
                  style={{ color: "var(--accent-primary)" }}
                  onClick={(e) => e.stopPropagation()}
                >
                  Release notes ↗
                </a>
              )}
            </div>

            {updateBannerState === "downloading" && (
              <div className="flex items-center gap-2 min-w-[110px]">
                <div className="flex-1 h-1.5 bg-[var(--bg-surface)] rounded-full overflow-hidden border border-[var(--border-subtle)]">
                  <div
                    className="h-full rounded-full transition-all duration-200"
                    style={{
                      width: `${updateBannerProgress}%`,
                      backgroundColor: "var(--accent-primary)",
                    }}
                  />
                </div>
                <span className="text-[11px] text-[var(--text-secondary)] font-medium tabular-nums shrink-0">
                  {updateBannerProgress}%
                </span>
              </div>
            )}
            {updateBannerState === "installing" && (
              <span className="text-[11px] text-[var(--text-secondary)] animate-pulse whitespace-nowrap">
                Installing...
              </span>
            )}
            {updateBannerState === "failed" && (
              <span className="text-[11px] text-rose-500 whitespace-nowrap" title={updateBannerError || ""}>
                {updateBannerError || "Installation failed"}
              </span>
            )}

            {/* Action Buttons */}
            <div className="flex items-center gap-1.5 shrink-0 pl-1 border-l border-[var(--border-subtle)]">
              {updateBannerState === "failed" && (
                <button
                  onClick={() => tauriOpenInstallerFolder()}
                  className="px-2.5 py-1 rounded-md text-[11px] font-medium text-[var(--text-primary)] bg-[var(--bg-surface)] border border-[var(--border-subtle)] hover:bg-[var(--bg-hover)] cursor-pointer flex items-center gap-1 transition-colors"
                  title="Open installer folder in File Explorer"
                >
                  <FolderOpen className="w-3 h-3" />
                  <span>Open folder</span>
                </button>
              )}
              <button
                onClick={handleBannerUpdate}
                disabled={updateBannerState === "downloading" || updateBannerState === "installing"}
                className="px-3 py-1 rounded-md text-[11px] font-semibold text-white cursor-pointer flex items-center gap-1.5 transition-opacity hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed"
                style={{ backgroundColor: "var(--accent-primary)" }}
                aria-label="Install software update"
              >
                {updateBannerState === "downloading" ? (
                  <span>Downloading...</span>
                ) : updateBannerState === "installing" ? (
                  <span>Restarting...</span>
                ) : updateBannerState === "failed" ? (
                  <span>Retry</span>
                ) : (
                  <span>Update Now</span>
                )}
              </button>
              <button
                onClick={handleDismissUpdateBanner}
                className="p-1 rounded-md text-[var(--text-muted)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
                title="Dismiss update banner"
                aria-label="Dismiss update banner"
              >
                <X className="w-3.5 h-3.5" />
              </button>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

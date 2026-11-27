import React, { useCallback, useEffect, useState } from "react";
import { X, CheckCircle2, FolderOpen, FileCode, Search, AlertCircle, Film } from "lucide-react";
import { EdmSettings, loadSettings } from "../types/settings";
import { Select } from "./Select";
import { applyFontFamily } from "../lib/fonts";
import {
  isTauri,
  TorrentInspectionResult,
  tauriCheckDuplicateDownload,
  tauriInspectTorrentFiles,
  tauriLoadSettings,
  tauriProbeMediaStreams,
  tauriProbeUrl,
  tauriSelectDownloadDirectory,
  tauriSelectTorrentFile,
  tauriStartDownload,
} from "../services/tauriBridge";
import {
  TorrentPowerOptions,
  DEFAULT_TORRENT_POWER_CONFIG,
  TorrentPowerConfig,
} from "../features/torrent-options";
import {
  MediaStreamPickerModal,
  MediaProbeInfo,
  MediaPickerSelection,
} from "../features/media-picker";

interface CapturePayload {
  url: string;
  filename?: string | null;
  size?: number | null;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

function readInitialCaptureFromLocation(): CapturePayload {
  if (typeof window === "undefined") return { url: "" };
  const params = new URLSearchParams(window.location.search);
  const url = params.get("url") || "";
  const filename = params.get("filename");
  const rawSize = params.get("size");
  const parsedSize = rawSize ? Number(rawSize) : NaN;
  return {
    url,
    filename: filename || undefined,
    size: Number.isFinite(parsedSize) && parsedSize > 0 ? parsedSize : null,
  };
}

function deriveFilenameFromUrl(rawUrl: string): string {
  const cleanUrl = rawUrl.trim();
  if (!cleanUrl) return "";
  try {
    const parsed = new URL(cleanUrl);
    const pathname = parsed.pathname;
    const lastPart = pathname.substring(pathname.lastIndexOf("/") + 1);
    if (lastPart && !lastPart.includes("?") && lastPart.length > 2) {
      return decodeURIComponent(lastPart);
    }
  } catch {
    const parts = cleanUrl.split("/");
    const lastPart = parts[parts.length - 1]?.split("?")[0];
    if (lastPart && lastPart.length > 2) {
      return lastPart;
    }
  }
  return "";
}

export default function AddDownloadPopup() {
  const initialCapture = React.useMemo(() => readInitialCaptureFromLocation(), []);
  const [settings, setSettings] = useState<EdmSettings>(() => loadSettings());

  const getDefaultConnectionsOption = useCallback((s: EdmSettings): number => {
    if (s.downloads.segmentedDownloads === "disabled") return 1;
    if (s.downloads.autoAdaptiveStreams !== false) return 0;
    const ceiling = s.downloads.maxConnectionsPerFile;
    if (ceiling === 1 || ceiling === 16 || ceiling === 32 || ceiling === 64) {
      return ceiling;
    }
    return 0;
  }, []);

  const [urlInput, setUrlInput] = useState(initialCapture.url);
  const [fileNameInput, setFileNameInput] = useState(
    initialCapture.filename?.trim() || deriveFilenameFromUrl(initialCapture.url)
  );
  const [addCategory, setAddCategory] = useState("auto");
  const [addConnections, setAddConnections] = useState<number>(() =>
    getDefaultConnectionsOption(settings)
  );
  const [addSaveDir, setAddSaveDir] = useState<string>("");
  const [availableQueues, setAvailableQueues] = useState<Array<{ id: string; name: string }>>([]);
  const [selectedQueueId, setSelectedQueueId] = useState<string>("immediate");

  const [isProbing, setIsProbing] = useState(false);
  const [probeResult, setProbeResult] = useState<{ size: string; finalUrl: string } | null>(() =>
    initialCapture.size && initialCapture.size > 0
      ? { size: formatBytes(initialCapture.size), finalUrl: initialCapture.url }
      : null
  );
  const [isInspectingTorrent, setIsInspectingTorrent] = useState(false);
  const [torrentInfo, setTorrentInfo] = useState<TorrentInspectionResult | null>(null);
  const [torrentInspectError, setTorrentInspectError] = useState<string | null>(null);
  const [selectedFileIndices, setSelectedFileIndices] = useState<Set<number>>(new Set());
  const [torrentPowerConfig, setTorrentPowerConfig] = useState<TorrentPowerConfig>(
    DEFAULT_TORRENT_POWER_CONFIG
  );
  const [isMediaPickerOpen, setIsMediaPickerOpen] = useState(false);
  const [mediaProbeInfo, setMediaProbeInfo] = useState<MediaProbeInfo | null>(null);
  const [isMediaProbing, setIsMediaProbing] = useState(false);
  const [mediaProbeError, setMediaProbeError] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  const closePopup = useCallback(async () => {
    if (!isTauri()) {
      window.close();
      return;
    }
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      await getCurrentWindow().close();
    } catch (err) {
      console.warn("Failed to close add-download popup:", err);
    }
  }, []);

  const applyCapture = useCallback((capture: CapturePayload) => {
    if (!capture.url) return;
    setUrlInput(capture.url);
    const derived = capture.filename?.trim() || deriveFilenameFromUrl(capture.url);
    if (derived) setFileNameInput(derived);
    if (capture.size && capture.size > 0) {
      setProbeResult({ size: formatBytes(capture.size), finalUrl: capture.url });
    } else {
      setProbeResult(null);
    }
    setTorrentInfo(null);
    setTorrentInspectError(null);
    setSelectedFileIndices(new Set());
  }, []);

  useEffect(() => {
    const syncAppearanceAndSettings = async () => {
      const local = loadSettings();
      const remote = isTauri() ? await tauriLoadSettings().catch(() => null) : null;
      const merged = remote ?? local;
      setSettings(merged);
      setAddConnections(getDefaultConnectionsOption(merged));
      const g = merged.general;
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
    void syncAppearanceAndSettings();

    try {
      const raw = localStorage.getItem("edm_queues");
      const parsed = raw ? JSON.parse(raw) : [];
      const list = Array.isArray(parsed)
        ? parsed.map((q: any) => ({ id: String(q.id), name: String(q.name || "Unnamed Queue") }))
        : [];
      setAvailableQueues(list);
    } catch {
      setAvailableQueues([]);
    }
  }, [getDefaultConnectionsOption]);

  useEffect(() => {
    if (!isTauri()) return;
    let mounted = true;
    let unlisten: (() => void) | undefined;
    import("@tauri-apps/api/event")
      .then(({ listen }) =>
        listen<CapturePayload>("prompt-add-download", (event) => {
          if (event.payload?.url) {
            applyCapture(event.payload);
          }
        })
      )
      .then((fn) => {
        if (mounted) unlisten = fn;
        else fn();
      })
      .catch(() => {});
    return () => {
      mounted = false;
      if (unlisten) unlisten();
    };
  }, [applyCapture]);

  const lastAutoInspectedRef = React.useRef<string>("");

  useEffect(() => {
    const trimmed = urlInput.trim();
    const lower = trimmed.toLowerCase();
    const isTorrent = lower.startsWith("magnet:") || lower.split("?")[0].endsWith(".torrent");
    if (isTorrent && trimmed !== lastAutoInspectedRef.current) {
      lastAutoInspectedRef.current = trimmed;
      void handleInspectTorrent(trimmed);
    } else if (!isTorrent && lastAutoInspectedRef.current) {
      lastAutoInspectedRef.current = "";
    }
  }, [urlInput]);

  useEffect(() => {
    if (!isTauri()) return;
    const hasFiles = Boolean(torrentInfo && torrentInfo.files.length > 0);
    import("@tauri-apps/api/window")
      .then(async ({ getCurrentWindow, LogicalSize }) => {
        await getCurrentWindow().setSize(new LogicalSize(540, hasFiles ? 600 : 430));
      })
      .catch(() => {});
  }, [torrentInfo]);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !isMediaPickerOpen) {
        void closePopup();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [closePopup, isMediaPickerOpen]);

  const trimmedUrl = urlInput.trim();
  const lowerUrl = trimmedUrl.toLowerCase();
  const isTorrentTarget =
    lowerUrl.startsWith("magnet:") || lowerUrl.split("?")[0].endsWith(".torrent");

  const detectCategoryName = (fname: string): string => {
    const ext = fname.split(".").pop()?.toLowerCase() || "";
    if (["mp4", "mkv", "webm", "avi", "mov", "flv", "wmv"].includes(ext)) return "Video";
    if (["mp3", "flac", "wav", "aac", "ogg", "m4a"].includes(ext)) return "Music";
    if (["zip", "rar", "7z", "tar", "gz", "iso", "bin"].includes(ext)) return "Compressed";
    if (["pdf", "doc", "docx", "txt", "xlsx", "pptx"].includes(ext)) return "Documents";
    if (["jpg", "jpeg", "png", "gif", "webp", "avif", "svg", "bmp", "ico", "tiff", "tif"].includes(ext)) {
      return "Pictures";
    }
    const matched = settings.categories.find((c) => c.enabled && c.extensions.includes(ext));
    return matched ? matched.name : "Programs";
  };

  const effectiveCategory =
    addCategory === "auto" ? detectCategoryName(fileNameInput || urlInput) : addCategory;

  const getAutoDestination = (catName: string): string => {
    if (settings.storage.folderMode === "auto") {
      const cat = settings.categories.find(
        (c) =>
          c.enabled &&
          (c.name.toLowerCase() === catName.toLowerCase() ||
            c.id.toLowerCase() === catName.toLowerCase() ||
            (catName === "Compressed" && (c.id === "archives" || c.name === "Archives")) ||
            (catName === "Music" && (c.id === "audio" || c.name === "Audio")))
      );
      if (cat?.destinationDirectory) return cat.destinationDirectory;
    }
    return settings.downloads.defaultDirectory;
  };

  const currentDestination = addSaveDir || getAutoDestination(effectiveCategory);

  const handleUrlInputChange = (val: string) => {
    setUrlInput(val);
    setTorrentInfo(null);
    setTorrentInspectError(null);
    setSelectedFileIndices(new Set());
    if (!val.trim()) {
      setFileNameInput("");
      setProbeResult(null);
      return;
    }
    const derived = deriveFilenameFromUrl(val);
    if (derived) setFileNameInput(derived);
  };

  const handleProbe = async () => {
    if (!trimmedUrl) return;
    setIsProbing(true);
    try {
      if (isTauri()) {
        const res = await tauriProbeUrl(trimmedUrl);
        if (res) {
          if (res.filename && !fileNameInput) {
            setFileNameInput(res.filename);
          }
          const sizeStr = res.size_total
            ? `${(res.size_total / (1024 * 1024)).toFixed(1)} MB`
            : "Unknown / Stream";
          setProbeResult({
            size: sizeStr,
            finalUrl: res.final_url || trimmedUrl,
          });
        }
      } else {
        setProbeResult({
          size: "420.5 MB (Simulated)",
          finalUrl: trimmedUrl,
        });
      }
    } catch (e) {
      console.error("Probe error:", e);
    } finally {
      setIsProbing(false);
    }
  };

  const handleProbeMedia = async () => {
    if (!trimmedUrl) return;
    setIsMediaPickerOpen(true);
    setIsMediaProbing(true);
    setMediaProbeError(null);
    try {
      const info = await tauriProbeMediaStreams(trimmedUrl);
      setMediaProbeInfo(info);
    } catch (e: any) {
      setMediaProbeInfo(null);
      setMediaProbeError(typeof e === "string" ? e : e?.message || "Failed to probe media streams");
    } finally {
      setIsMediaProbing(false);
    }
  };

  const handleInspectTorrent = async (targetUrlOverride?: string) => {
    const target = (targetUrlOverride ?? urlInput).trim();
    if (!target) return;
    setIsInspectingTorrent(true);
    setTorrentInspectError(null);
    setProbeResult(null);
    try {
      const info = await tauriInspectTorrentFiles(target, currentDestination);
      setTorrentInfo(info);
      setSelectedFileIndices(new Set(info.files.map((f) => f.index)));
      if (info.name && (!fileNameInput.trim() || target.toLowerCase().startsWith("magnet:"))) {
        setFileNameInput(info.name);
      }
    } catch (e: any) {
      setTorrentInfo(null);
      setSelectedFileIndices(new Set());
      setTorrentInspectError(
        typeof e === "string" ? e : e?.message || "Failed to inspect torrent metadata"
      );
    } finally {
      setIsInspectingTorrent(false);
    }
  };

  const handleBrowseAddDir = async () => {
    try {
      const chosen = await tauriSelectDownloadDirectory(currentDestination);
      if (chosen) {
        setAddSaveDir(chosen);
      }
    } catch (e) {
      console.error("Failed to select download directory:", e);
    }
  };

  const handleBrowseTorrentFile = async () => {
    try {
      const chosen = await tauriSelectTorrentFile();
      if (chosen) {
        setUrlInput(chosen);
        const name = chosen.split(/[\\/]/).pop() || "";
        setFileNameInput(name);
        setAddCategory("Compressed");
        void handleInspectTorrent(chosen);
      }
    } catch (err) {
      console.warn("Failed to select torrent file:", err);
    }
  };

  const toggleTorrentFile = (index: number) => {
    setSelectedFileIndices((prev) => {
      const next = new Set(prev);
      if (next.has(index)) next.delete(index);
      else next.add(index);
      return next;
    });
  };

  const selectedFilesList = torrentInfo
    ? torrentInfo.files.filter((f) => selectedFileIndices.has(f.index))
    : [];
  const selectedTotalBytes = selectedFilesList.reduce((acc, f) => acc + f.size_bytes, 0);
  const noFilesSelected = Boolean(
    torrentInfo && torrentInfo.files.length > 0 && selectedFileIndices.size === 0
  );

  const startSingleDownload = async (
    targetUrl: string,
    rawFilename: string,
    queueOnly: boolean,
    selectedFiles?: number[]
  ) => {
    const isYouTubeUrl = targetUrl.includes("youtube.com") || targetUrl.includes("youtu.be");
    let initialName =
      (rawFilename && rawFilename.trim()) ||
      targetUrl.split("/").pop()?.split("?")[0] ||
      "download.bin";
    if (
      isYouTubeUrl &&
      (!rawFilename.trim() || initialName === "watch" || initialName === "watch.mp4")
    ) {
      initialName = "";
    }
    const safeName = initialName
      ? initialName.replace(/[\\/:*?"<>|]/g, "_")
      : isYouTubeUrl
        ? "YouTube_Video.mp4"
        : "download.bin";

    const dupCheck = await tauriCheckDuplicateDownload(targetUrl, safeName, 0);
    if (dupCheck.is_duplicate) {
      const proceed = window.confirm(
        `Duplicate Download Detected:\n${
          dupCheck.existing_filename
            ? `File already exists as ${dupCheck.existing_filename} (${dupCheck.existing_state})`
            : "This download already exists in tasks."
        }\n\nDo you still wish to add it again?`
      );
      if (!proceed) return false;
    }

    await tauriStartDownload({
      url: targetUrl,
      filename: safeName,
      saveDir: currentDestination,
      maxConnections: addConnections,
      selectedFiles,
      queueOnly,
    });
    return true;
  };

  const handleMediaConfirm = async (selection: MediaPickerSelection) => {
    if (!mediaProbeInfo) return;
    if (selection.selectedEntryUrls.length > 0) {
      setIsSubmitting(true);
      try {
        for (const entryUrl of selection.selectedEntryUrls) {
          const entry = mediaProbeInfo.entries.find((e) => e.url === entryUrl);
          await startSingleDownload(
            entryUrl,
            entry?.title ? `${entry.title}.mp4` : "",
            false
          );
        }
        await closePopup();
      } finally {
        setIsSubmitting(false);
      }
      return;
    }

    const chosenVideo = mediaProbeInfo.video_formats.find(
      (f) => f.format_id === selection.videoFormatId
    );
    const chosenAudio = mediaProbeInfo.audio_formats.find(
      (f) => f.format_id === selection.audioFormatId
    );
    const directUrl = chosenVideo?.url || chosenAudio?.url;
    if (directUrl) setUrlInput(directUrl);
    if (mediaProbeInfo.title && !fileNameInput.trim()) {
      const ext = chosenVideo?.ext || chosenAudio?.ext || "mp4";
      setFileNameInput(`${mediaProbeInfo.title}.${ext}`);
    }
    setAddCategory("Video");
    setIsMediaPickerOpen(false);
  };

  const handleSubmit = async (queueOnly = false) => {
    if (!trimmedUrl || noFilesSelected || isSubmitting) return;
    setIsSubmitting(true);
    try {
      const selectedIndices =
        torrentInfo && torrentInfo.files.length > 0
          ? Array.from(selectedFileIndices).sort((a, b) => a - b)
          : undefined;
      const started = await startSingleDownload(
        trimmedUrl,
        fileNameInput.trim(),
        queueOnly || selectedQueueId !== "immediate",
        selectedIndices
      );
      if (started) {
        await closePopup();
      }
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <div className="h-screen w-screen bg-transparent p-1.5 select-none">
      <div
        role="dialog"
        aria-modal="true"
        aria-label="Add Download"
        className="flex h-full w-full flex-col overflow-hidden rounded-xl bg-[var(--bg-panel)] border border-[var(--border-subtle)] text-[var(--text-primary)] shadow-[0_24px_48px_-12px_rgba(0,0,0,0.75)]"
      >
        <div
          data-tauri-drag-region
          className="px-4 py-2.5 border-b border-[var(--border-dim)] flex items-center justify-between shrink-0 cursor-move"
        >
          <span
            data-tauri-drag-region
            className="font-semibold tracking-tight text-xs sm:text-sm text-[var(--text-primary)] flex items-center gap-2 pointer-events-none"
          >
            <img src="/logo.png" alt="EDM Logo" className="w-4 h-4 object-contain" />
            {selectedQueueId !== "immediate" ? "Add to Scheduler" : "Add Download"}
          </span>
          <button
            type="button"
            onClick={() => void closePopup()}
            aria-label="Close Add Download dialog"
            className="w-7 h-7 rounded-lg flex items-center justify-center text-[var(--text-muted)] hover:text-[var(--text-primary)] hover:bg-white/[0.06] transition-colors cursor-pointer"
          >
            <X className="w-4 h-4 stroke-[1.75]" />
          </button>
        </div>

        <div className="flex-1 overflow-y-auto p-4 space-y-3">
          <div>
            <div className="flex items-center justify-between mb-1.5 gap-2">
              <label className="block text-xs font-medium text-[var(--text-secondary)]">
                Source URL
              </label>
              <button
                type="button"
                onClick={() => void handleBrowseTorrentFile()}
                className="text-[11px] text-[var(--text-secondary)] hover:text-[var(--accent)] inline-flex items-center gap-1 cursor-pointer font-medium transition-colors"
              >
                <FileCode className="w-3.5 h-3.5 stroke-[1.75]" />
                <span>.torrent File</span>
              </button>
            </div>
            <div className="flex gap-2">
              <input
                type="text"
                placeholder="https://example.com/file.zip or magnet:?xt=..."
                value={urlInput}
                onChange={(e) => handleUrlInputChange(e.target.value)}
                className="flex-1 h-8 px-3 rounded-lg text-xs bg-[var(--bg-chassis)] border border-[var(--border-subtle)] text-[var(--text-primary)] placeholder-[var(--text-muted)] focus:outline-none focus:border-[var(--accent)] transition-colors"
              />
              {isTorrentTarget ? (
                <button
                  type="button"
                  onClick={() => void handleInspectTorrent()}
                  disabled={isInspectingTorrent || !trimmedUrl}
                  className="h-8 px-3 rounded-lg bg-white/[0.04] border border-[var(--border-subtle)] text-xs font-medium text-[var(--accent)] hover:bg-white/[0.08] disabled:opacity-40 cursor-pointer inline-flex items-center gap-1.5 shrink-0 transition-colors"
                >
                  <Search className="w-3.5 h-3.5 stroke-[1.75]" />
                  <span>{isInspectingTorrent ? "Looking up..." : "Look up"}</span>
                </button>
              ) : (
                <>
                  <button
                    type="button"
                    onClick={() => void handleProbe()}
                    disabled={isProbing || !trimmedUrl}
                    className="h-8 px-2.5 rounded-lg bg-white/[0.04] border border-[var(--border-subtle)] text-xs font-medium text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-white/[0.08] disabled:opacity-40 cursor-pointer shrink-0 transition-colors"
                  >
                    {isProbing ? "Probing..." : "Probe"}
                  </button>
                  <button
                    type="button"
                    onClick={() => void handleProbeMedia()}
                    disabled={isMediaProbing || !trimmedUrl}
                    className="btn-3d px-3 py-1.5 rounded-lg text-xs font-semibold text-[var(--accent)] disabled:opacity-40 cursor-pointer inline-flex items-center gap-1.5 shrink-0"
                    title="Probe video/audio/subtitle streams (yt-dlp / DASH)"
                  >
                    <Film className="w-3.5 h-3.5" />
                    <span>{isMediaProbing ? "Probing..." : "Probe Media"}</span>
                  </button>
                </>
              )}
            </div>
          </div>

          <div>
            <label className="block text-xs font-medium text-[var(--text-secondary)] mb-1">
              File Name
            </label>
            <input
              type="text"
              placeholder="Enter file name (e.g. video.mp4)..."
              value={fileNameInput}
              onChange={(e) => setFileNameInput(e.target.value)}
              className="w-full px-3 py-2 rounded-lg text-xs input-3d text-[var(--text-primary)] focus:outline-none font-medium"
            />
          </div>

          {probeResult && (
            <div className="p-2.5 rounded-xl card-3d border border-[var(--accent-badge-border)] text-xs space-y-0.5">
              <div className="font-semibold text-[var(--accent)] flex items-center gap-1.5">
                <CheckCircle2 className="w-3.5 h-3.5 icon-3d" /> Endpoint Validated & Resumable
              </div>
              <div className="text-[11px] text-[var(--text-secondary)] font-mono">
                Size: {probeResult.size} • Accept-Ranges: bytes
              </div>
            </div>
          )}

          {torrentInspectError && (
            <div className="p-2.5 rounded-xl card-3d border border-red-500/40 text-xs flex items-start gap-2 text-red-400">
              <AlertCircle className="w-4 h-4 shrink-0 mt-0.5" />
              <span className="break-words">{torrentInspectError}</span>
            </div>
          )}

          {torrentInfo && torrentInfo.files.length > 0 && (
            <div className="p-2.5 rounded-xl card-3d border border-[var(--border-subtle)] text-xs space-y-2">
              <div className="flex items-center justify-between gap-2">
                <div className="font-semibold text-[var(--text-primary)] truncate">
                  {torrentInfo.name || "Torrent Contents"}
                </div>
                <div className="flex items-center gap-2 shrink-0">
                  <button
                    type="button"
                    onClick={() => setSelectedFileIndices(new Set(torrentInfo.files.map((f) => f.index)))}
                    className="text-[11px] font-medium text-[var(--accent)] hover:underline cursor-pointer"
                  >
                    Select all
                  </button>
                  <span className="text-[var(--text-muted)]">•</span>
                  <button
                    type="button"
                    onClick={() => setSelectedFileIndices(new Set())}
                    className="text-[11px] font-medium text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:underline cursor-pointer"
                  >
                    Select none
                  </button>
                </div>
              </div>
              <div className="max-h-28 overflow-y-auto space-y-1 pr-1">
                {torrentInfo.files.map((file) => (
                  <label
                    key={file.index}
                    className="flex items-center justify-between gap-2 px-2 py-1 rounded-lg hover:bg-white/5 cursor-pointer"
                  >
                    <div className="flex items-center gap-2 min-w-0">
                      <input
                        type="checkbox"
                        checked={selectedFileIndices.has(file.index)}
                        onChange={() => toggleTorrentFile(file.index)}
                        className="accent-[var(--accent)] cursor-pointer shrink-0"
                      />
                      <span className="text-[11px] text-[var(--text-primary)] truncate font-mono" title={file.path}>
                        {file.path}
                      </span>
                    </div>
                    <span className="text-[11px] text-[var(--text-secondary)] font-mono shrink-0">
                      {formatBytes(file.size_bytes)}
                    </span>
                  </label>
                ))}
              </div>
              <div className="pt-1 border-t border-[var(--border-subtle)] flex items-center justify-between text-[11px] text-[var(--text-secondary)] font-mono">
                <span>
                  Selected: {selectedFilesList.length} {selectedFilesList.length === 1 ? "file" : "files"},{" "}
                  {(selectedTotalBytes / (1024 * 1024)).toFixed(1)} MB
                </span>
                <span>Total: {formatBytes(torrentInfo.total_bytes)}</span>
              </div>
            </div>
          )}

          {isTorrentTarget && (
            <TorrentPowerOptions value={torrentPowerConfig} onChange={setTorrentPowerConfig} />
          )}

          <div className="grid grid-cols-2 gap-3 text-xs">
            <div>
              <label className="block font-medium text-[var(--text-secondary)] mb-1">Category</label>
              <Select
                value={addCategory}
                onChange={setAddCategory}
                ariaLabel="Download Category"
                className="w-full"
                options={[
                  { value: "auto", label: "Auto-Detect from Extension" },
                  ...settings.categories
                    .filter((c) => c.enabled)
                    .map((c) => ({ value: c.name, label: c.name })),
                ]}
              />
            </div>
            <div>
              <label className="block font-medium text-[var(--text-secondary)] mb-1">Stream Scaling</label>
              <Select
                value={String(addConnections)}
                onChange={(v) => setAddConnections(Number(v))}
                ariaLabel="Stream Scaling"
                className="w-full"
                options={[
                  { value: "0", label: "Auto (Adaptive — Recommended)" },
                  { value: "1", label: "Single Stream (Strict Compatibility)" },
                  { value: "16", label: "16 Max" },
                  { value: "32", label: "32 Max" },
                  { value: "64", label: "64 Max" },
                ]}
              />
            </div>
          </div>

          <div>
            <label className="block text-xs font-medium text-[var(--text-secondary)] mb-1">
              Schedule / Queue
            </label>
            <Select
              value={selectedQueueId}
              onChange={setSelectedQueueId}
              ariaLabel="Schedule Queue"
              className="w-full"
              options={[
                { value: "immediate", label: "Immediate (Download now)" },
                ...availableQueues.map((q) => ({
                  value: q.id,
                  label: `Queue: ${q.name}`,
                })),
                ...(availableQueues.length === 0
                  ? [{ value: "default-queue", label: "Default Scheduled Queue" }]
                  : []),
              ]}
            />
          </div>

          <div>
            <label className="block text-xs font-medium text-[var(--text-secondary)] mb-1">
              Destination Directory
            </label>
            <div className="flex gap-2">
              <input
                type="text"
                value={currentDestination}
                onChange={(e) => setAddSaveDir(e.target.value)}
                className="flex-1 h-8 px-3 rounded-lg text-xs bg-[var(--bg-chassis)] border border-[var(--border-subtle)] text-[var(--text-primary)] font-mono focus:outline-none focus:border-[var(--accent)] transition-colors"
              />
              <button
                type="button"
                onClick={() => void handleBrowseAddDir()}
                className="h-8 px-3 rounded-lg bg-white/[0.04] border border-[var(--border-subtle)] text-xs font-medium text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-white/[0.08] cursor-pointer inline-flex items-center gap-1.5 shrink-0 transition-colors"
              >
                <FolderOpen className="w-3.5 h-3.5 stroke-[1.75] text-[var(--accent)]" />
                <span>Browse</span>
              </button>
            </div>
          </div>
        </div>

        <div className="px-4 py-2.5 border-t border-[var(--border-dim)] flex items-center justify-between gap-2 shrink-0">
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={() => void handleSubmit(true)}
              disabled={!trimmedUrl || noFilesSelected || isSubmitting}
              className="h-8 px-3.5 rounded-lg bg-white/[0.04] border border-[var(--border-subtle)] text-xs font-medium text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-white/[0.08] disabled:opacity-40 cursor-pointer transition-colors"
              title="Add to queue without starting immediately"
            >
              Add
            </button>
            <button
              type="button"
              onClick={() => void handleSubmit(false)}
              disabled={!trimmedUrl || noFilesSelected || isSubmitting}
              className="h-8 px-4 rounded-lg bg-[var(--btn-contrast-bg)] hover:bg-[var(--btn-contrast-hover)] text-[var(--btn-contrast-text)] text-xs font-medium disabled:opacity-40 cursor-pointer transition-colors"
            >
              {selectedQueueId !== "immediate" ? "Add to Scheduler" : "Download"}
            </button>
          </div>
          <button
            type="button"
            onClick={() => void closePopup()}
            className="h-8 px-3 rounded-lg bg-white/[0.04] border border-[var(--border-subtle)] text-xs font-medium text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-white/[0.08] cursor-pointer transition-colors"
          >
            Cancel
          </button>
        </div>
      </div>

      <MediaStreamPickerModal
        isOpen={isMediaPickerOpen}
        onClose={() => setIsMediaPickerOpen(false)}
        probeInfo={mediaProbeInfo}
        isLoading={isMediaProbing}
        error={mediaProbeError}
        onConfirm={handleMediaConfirm}
      />
    </div>
  );
}

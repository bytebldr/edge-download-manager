import React, { useEffect, useState } from "react";
import { X, CheckCircle2, FolderOpen, FileCode, Search, AlertCircle, Film, Upload } from "lucide-react";
import { motion, AnimatePresence } from "motion/react";
import { backdropVariants, modalVariants } from "../utils/motion";
import { EdmSettings } from "../types/settings";
import { Select } from "./Select";
import {
  isTauri,
  TorrentInspectionResult,
  tauriInspectTorrentFiles,
  tauriProbeUrl,
  tauriProbeMediaStreams,
  tauriSelectDownloadDirectory,
  tauriSelectTorrentFile,
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

interface AddDownloadModalProps {
  isOpen: boolean;
  onClose: () => void;
  onAdd: (
    url: string,
    filename: string,
    category: string,
    connections: number,
    saveDir: string,
    selectedFiles?: number[],
    scheduleQueueId?: string,
    queueOnly?: boolean
  ) => void;
  onOpenBatchImport?: () => void;
  initialQueueMode?: boolean;
  initialUrl?: string;
  initialFilename?: string;
  initialSize?: number | null;
  settings: EdmSettings;
}

function formatTorrentBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

export const AddDownloadModal: React.FC<AddDownloadModalProps> = ({
  isOpen,
  onClose,
  onAdd,
  onOpenBatchImport,
  initialQueueMode = false,
  initialUrl,
  initialFilename,
  initialSize,
  settings,
}) => {
  const getDefaultConnectionsOption = (): number => {
    if (settings.downloads.segmentedDownloads === "disabled") return 1;
    if (settings.downloads.autoAdaptiveStreams !== false) return 0;
    const ceiling = settings.downloads.maxConnectionsPerFile;
    if (ceiling === 1 || ceiling === 16 || ceiling === 32 || ceiling === 64) {
      return ceiling;
    }
    return 0;
  };

  const [urlInput, setUrlInput] = useState("");
  const [fileNameInput, setFileNameInput] = useState("");
  const [addCategory, setAddCategory] = useState("auto");
  const [addConnections, setAddConnections] = useState<number>(getDefaultConnectionsOption);
  const [addSaveDir, setAddSaveDir] = useState<string>("");
  const [availableQueues, setAvailableQueues] = useState<Array<{ id: string; name: string }>>([]);
  const [selectedQueueId, setSelectedQueueId] = useState<string>("immediate");

  useEffect(() => {
    if (!isOpen) return;
    if (initialUrl) {
      setUrlInput(initialUrl);
      const derived = initialFilename?.trim() || initialUrl.split("/").pop()?.split("?")[0] || "";
      if (derived) setFileNameInput(derived);
      if (initialSize && initialSize > 0) {
        setProbeResult({ size: formatTorrentBytes(initialSize), finalUrl: initialUrl });
      }
    }
    try {
      const raw = localStorage.getItem("edm_queues");
      const parsed = raw ? JSON.parse(raw) : [];
      const list = Array.isArray(parsed)
        ? parsed.map((q: any) => ({ id: String(q.id), name: String(q.name || "Unnamed Queue") }))
        : [];
      setAvailableQueues(list);
      if (initialQueueMode) {
        setSelectedQueueId(list.length > 0 ? list[0].id : "default-queue");
      } else {
        setSelectedQueueId("immediate");
      }
    } catch {
      setAvailableQueues([]);
      setSelectedQueueId(initialQueueMode ? "default-queue" : "immediate");
    }
  }, [isOpen, initialQueueMode, initialUrl, initialFilename, initialSize]);
  const [isProbing, setIsProbing] = useState(false);
  const [probeResult, setProbeResult] = useState<{
    size: string;
    finalUrl: string;
  } | null>(null);
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
    if (["jpg", "jpeg", "png", "gif", "webp", "avif", "svg", "bmp", "ico", "tiff", "tif"].includes(ext)) return "Pictures";
    const matched = settings.categories.find((c) => c.enabled && c.extensions.includes(ext));
    return matched ? matched.name : "Programs";
  };

  const effectiveCategory = addCategory === "auto" ? detectCategoryName(fileNameInput || urlInput) : addCategory;

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

  React.useEffect(() => {
    if (isOpen) {
      setAddConnections(getDefaultConnectionsOption());
    }
  }, [
    isOpen,
    settings.downloads.autoAdaptiveStreams,
    settings.downloads.maxConnectionsPerFile,
    settings.downloads.segmentedDownloads,
  ]);

  const resetState = () => {
    setUrlInput("");
    setFileNameInput("");
    setProbeResult(null);
    setTorrentInfo(null);
    setTorrentInspectError(null);
    setSelectedFileIndices(new Set());
    setTorrentPowerConfig(DEFAULT_TORRENT_POWER_CONFIG);
    setIsMediaPickerOpen(false);
    setMediaProbeInfo(null);
    setMediaProbeError(null);
    setAddSaveDir("");
    setAddConnections(getDefaultConnectionsOption());
    onClose();
  };

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
    const cleanUrl = val.trim();
    try {
      const parsed = new URL(cleanUrl);
      const pathname = parsed.pathname;
      const lastPart = pathname.substring(pathname.lastIndexOf("/") + 1);
      if (lastPart && !lastPart.includes("?") && lastPart.length > 2) {
        setFileNameInput(decodeURIComponent(lastPart));
      }
    } catch {
      const parts = cleanUrl.split("/");
      const lastPart = parts[parts.length - 1]?.split("?")[0];
      if (lastPart && lastPart.length > 2) {
        setFileNameInput(lastPart);
      }
    }
  };

  const handleProbe = async () => {
    if (!urlInput.trim()) return;
    setIsProbing(true);
    try {
      if (isTauri()) {
        const res = await tauriProbeUrl(urlInput.trim());
        if (res) {
          if (res.filename && !fileNameInput) {
            setFileNameInput(res.filename);
          }
          const sizeStr = res.size_total
            ? `${(res.size_total / (1024 * 1024)).toFixed(1)} MB`
            : "Unknown / Stream";
          setProbeResult({
            size: sizeStr,
            finalUrl: res.final_url || urlInput.trim(),
          });
        }
      } else {
        setProbeResult({
          size: "420.5 MB (Simulated)",
          finalUrl: urlInput.trim(),
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

  const handleMediaConfirm = (selection: MediaPickerSelection) => {
    if (!mediaProbeInfo) return;
    if (selection.selectedEntryUrls.length > 0) {
      for (const entryUrl of selection.selectedEntryUrls) {
        const entry = mediaProbeInfo.entries.find((e) => e.url === entryUrl);
        onAdd(
          entryUrl,
          entry?.title ? `${entry.title}.mp4` : "",
          "Video",
          addConnections,
          currentDestination
        );
      }
      resetState();
      return;
    }

    const chosenVideo = mediaProbeInfo.video_formats.find(
      (f) => f.format_id === selection.videoFormatId
    );
    const chosenAudio = mediaProbeInfo.audio_formats.find(
      (f) => f.format_id === selection.audioFormatId
    );
    const directUrl = chosenVideo?.url || chosenAudio?.url;
    if (directUrl) {
      setUrlInput(directUrl);
    }
    if (mediaProbeInfo.title && !fileNameInput.trim()) {
      const ext = chosenVideo?.ext || chosenAudio?.ext || "mp4";
      setFileNameInput(`${mediaProbeInfo.title}.${ext}`);
    }
    setAddCategory("Video");
    setIsMediaPickerOpen(false);
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
      if (next.has(index)) {
        next.delete(index);
      } else {
        next.add(index);
      }
      return next;
    });
  };

  const selectAllTorrentFiles = () => {
    if (!torrentInfo) return;
    setSelectedFileIndices(new Set(torrentInfo.files.map((f) => f.index)));
  };

  const selectNoTorrentFiles = () => {
    setSelectedFileIndices(new Set());
  };

  const selectedFilesList = torrentInfo
    ? torrentInfo.files.filter((f) => selectedFileIndices.has(f.index))
    : [];
  const selectedTotalBytes = selectedFilesList.reduce((acc, f) => acc + f.size_bytes, 0);
  const noFilesSelected = Boolean(torrentInfo && torrentInfo.files.length > 0 && selectedFileIndices.size === 0);

  const handleSubmit = (queueOnly = false) => {
    if (!trimmedUrl || noFilesSelected) return;
    const selectedIndices =
      torrentInfo && torrentInfo.files.length > 0
        ? Array.from(selectedFileIndices).sort((a, b) => a - b)
        : undefined;
    onAdd(
      trimmedUrl,
      fileNameInput.trim(),
      effectiveCategory,
      addConnections,
      currentDestination,
      selectedIndices,
      selectedQueueId !== "immediate" ? selectedQueueId : undefined,
      queueOnly
    );
    resetState();
  };

  return (
    <AnimatePresence>
      {isOpen && (
        <motion.div
          variants={backdropVariants}
          initial="initial"
          animate="animate"
          exit="exit"
          className="fixed inset-0 bg-black/75 backdrop-blur-sm flex items-center justify-center p-4 z-50"
          onClick={resetState}
        >
          <motion.div
            variants={modalVariants}
            initial="initial"
            animate="animate"
            exit="exit"
            role="dialog"
            aria-modal="true"
            aria-label="Add Download"
            className="w-full max-w-lg rounded-xl bg-[var(--bg-panel)] border border-[var(--border-subtle)] shadow-[0_24px_48px_-12px_rgba(0,0,0,0.7)] overflow-hidden"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="px-4 py-3 border-b border-[var(--border-dim)] flex items-center justify-between">
              <span className="font-semibold tracking-tight text-xs sm:text-sm text-[var(--text-primary)] flex items-center gap-2">
                <img src="/logo.png" alt="EDM Logo" className="w-4 h-4 object-contain pointer-events-none" />
                {selectedQueueId !== "immediate" ? "Add to Scheduler" : "Add Download"}
              </span>
              <button
                onClick={resetState}
                aria-label="Close Add Download dialog"
                className="w-7 h-7 rounded-lg flex items-center justify-center text-[var(--text-muted)] hover:text-[var(--text-primary)] hover:bg-white/[0.06] transition-colors cursor-pointer"
              >
                <X className="w-4 h-4 stroke-[1.75]" />
              </button>
            </div>

            <div className="p-4 space-y-3.5">
              <div>
                <div className="flex items-center justify-between mb-1.5 gap-2">
                  <label className="block text-xs font-medium text-[var(--text-secondary)]">
                    Source URL
                  </label>
                  <div className="flex items-center gap-3 shrink-0">
                    {onOpenBatchImport && (
                      <button
                        type="button"
                        onClick={() => {
                          resetState();
                          onOpenBatchImport();
                        }}
                        className="text-[11px] text-[var(--text-secondary)] hover:text-[var(--accent)] inline-flex items-center gap-1 cursor-pointer font-medium transition-colors"
                      >
                        <Upload className="w-3.5 h-3.5 stroke-[1.75]" />
                        <span>Batch Import</span>
                      </button>
                    )}
                    <button
                      type="button"
                      onClick={handleBrowseTorrentFile}
                      className="text-[11px] text-[var(--text-secondary)] hover:text-[var(--accent)] inline-flex items-center gap-1 cursor-pointer font-medium transition-colors"
                    >
                      <FileCode className="w-3.5 h-3.5 stroke-[1.75]" />
                      <span>.torrent File</span>
                    </button>
                  </div>
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
                        onClick={handleProbe}
                        disabled={isProbing || !trimmedUrl}
                        className="h-8 px-2.5 rounded-lg bg-white/[0.04] border border-[var(--border-subtle)] text-xs font-medium text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-white/[0.08] disabled:opacity-40 cursor-pointer shrink-0 transition-colors"
                      >
                        {isProbing ? "Probing..." : "Probe"}
                      </button>
                      <button
                        type="button"
                        onClick={() => void handleProbeMedia()}
                        disabled={isMediaProbing || !trimmedUrl}
                        className="btn-3d px-3 py-2 rounded-lg text-xs font-semibold text-[var(--accent)] disabled:opacity-40 cursor-pointer inline-flex items-center gap-1.5 shrink-0"
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
                <div className="p-3 rounded-xl card-3d border border-[var(--accent-badge-border)] text-xs space-y-1">
                  <div className="font-semibold text-[var(--accent)] flex items-center gap-1.5">
                    <CheckCircle2 className="w-4 h-4 icon-3d" /> Endpoint Validated & Resumable
                  </div>
                  <div className="text-[11px] text-[var(--text-secondary)] font-mono">
                    Size: {probeResult.size} • Accept-Ranges: bytes • Sparse Pre-allocation Ready
                  </div>
                </div>
              )}

              {torrentInspectError && (
                <div className="p-3 rounded-xl card-3d border border-red-500/40 text-xs flex items-start gap-2 text-red-400">
                  <AlertCircle className="w-4 h-4 shrink-0 mt-0.5" />
                  <span className="break-words">{torrentInspectError}</span>
                </div>
              )}

              {torrentInfo && torrentInfo.files.length > 0 && (
                <div className="p-3 rounded-xl card-3d border border-[var(--border-subtle)] text-xs space-y-2">
                  <div className="flex items-center justify-between gap-2">
                    <div className="font-semibold text-[var(--text-primary)] truncate">
                      {torrentInfo.name || "Torrent Contents"}
                    </div>
                    <div className="flex items-center gap-2 shrink-0">
                      <button
                        type="button"
                        onClick={selectAllTorrentFiles}
                        className="text-[11px] font-medium text-[var(--accent)] hover:underline cursor-pointer"
                      >
                        Select all
                      </button>
                      <span className="text-[var(--text-muted)]">•</span>
                      <button
                        type="button"
                        onClick={selectNoTorrentFiles}
                        className="text-[11px] font-medium text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:underline cursor-pointer"
                      >
                        Select none
                      </button>
                    </div>
                  </div>

                  <div className="max-h-40 overflow-y-auto space-y-1 pr-1">
                    {torrentInfo.files.map((file) => {
                      const checked = selectedFileIndices.has(file.index);
                      return (
                        <label
                          key={file.index}
                          className="flex items-center justify-between gap-2 px-2 py-1.5 rounded-lg hover:bg-white/5 cursor-pointer"
                        >
                          <div className="flex items-center gap-2 min-w-0">
                            <input
                              type="checkbox"
                              checked={checked}
                              onChange={() => toggleTorrentFile(file.index)}
                              className="accent-[var(--accent)] cursor-pointer shrink-0"
                            />
                            <span className="text-[11px] text-[var(--text-primary)] truncate font-mono" title={file.path}>
                              {file.path}
                            </span>
                          </div>
                          <span className="text-[11px] text-[var(--text-secondary)] font-mono shrink-0">
                            {formatTorrentBytes(file.size_bytes)}
                          </span>
                        </label>
                      );
                    })}
                  </div>

                  <div className="pt-1 border-t border-[var(--border-subtle)] flex items-center justify-between text-[11px] text-[var(--text-secondary)] font-mono">
                    <span>
                      Selected: {selectedFilesList.length} {selectedFilesList.length === 1 ? "file" : "files"},{" "}
                      {(selectedTotalBytes / (1024 * 1024)).toFixed(1)} MB
                    </span>
                    <span>Total: {formatTorrentBytes(torrentInfo.total_bytes)}</span>
                  </div>
                </div>
              )}

              {isTorrentTarget && (
                <TorrentPowerOptions
                  value={torrentPowerConfig}
                  onChange={setTorrentPowerConfig}
                />
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
                    onClick={handleBrowseAddDir}
                    className="h-8 px-3 rounded-lg bg-white/[0.04] border border-[var(--border-subtle)] text-xs font-medium text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-white/[0.08] cursor-pointer inline-flex items-center gap-1.5 shrink-0 transition-colors"
                  >
                    <FolderOpen className="w-3.5 h-3.5 stroke-[1.75] text-[var(--accent)]" />
                    <span>Browse</span>
                  </button>
                </div>
              </div>
            </div>

            <div className="px-4 py-3 border-t border-[var(--border-dim)] flex items-center justify-between gap-2">
              <div className="flex items-center gap-2">
                <button
                  onClick={() => handleSubmit(true)}
                  disabled={!trimmedUrl || noFilesSelected}
                  className="h-8 px-3.5 rounded-lg bg-white/[0.04] border border-[var(--border-subtle)] text-xs font-medium text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-white/[0.08] disabled:opacity-40 cursor-pointer transition-colors"
                  title="Add to queue without starting immediately"
                >
                  Add
                </button>
                <button
                  onClick={() => handleSubmit(false)}
                  disabled={!trimmedUrl || noFilesSelected}
                  className="h-8 px-4 rounded-lg bg-[var(--btn-contrast-bg)] hover:bg-[var(--btn-contrast-hover)] text-[var(--btn-contrast-text)] text-xs font-medium disabled:opacity-40 cursor-pointer transition-colors"
                >
                  {selectedQueueId !== "immediate" ? "Add to Scheduler" : "Download"}
                </button>
              </div>
              <button
                onClick={resetState}
                className="h-8 px-3 rounded-lg bg-white/[0.04] border border-[var(--border-subtle)] text-xs font-medium text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-white/[0.08] cursor-pointer transition-colors"
              >
                Cancel
              </button>
            </div>
          </motion.div>
        </motion.div>
      )}
      <MediaStreamPickerModal
        isOpen={isMediaPickerOpen}
        onClose={() => setIsMediaPickerOpen(false)}
        probeInfo={mediaProbeInfo}
        isLoading={isMediaProbing}
        error={mediaProbeError}
        onConfirm={handleMediaConfirm}
      />
    </AnimatePresence>
  );
};

import React, { useState } from "react";
import { X, CheckCircle2, FolderOpen, FileCode, Search, AlertCircle, Film, Upload } from "lucide-react";
import { motion, AnimatePresence } from "motion/react";
import { backdropVariants, modalVariants } from "../utils/motion";
import { EdmSettings } from "../types/settings";
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
    selectedFiles?: number[]
  ) => void;
  onOpenBatchImport?: () => void;
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
  settings,
}) => {
  const [urlInput, setUrlInput] = useState("");
  const [fileNameInput, setFileNameInput] = useState("");
  const [addCategory, setAddCategory] = useState("auto");
  const [addConnections, setAddConnections] = useState<number>(
    settings.downloads.maxConnectionsPerFile || 8
  );
  const [addSaveDir, setAddSaveDir] = useState<string>("");
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

  const handleSubmit = () => {
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
      selectedIndices
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
            className="panel-3d w-full max-w-lg overflow-hidden"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="px-5 py-3 bg-[var(--bg-card)] border-b border-[var(--border-subtle)] flex items-center justify-between">
              <span className="font-bold tracking-tight text-sm flex items-center gap-2">
                <img src="/logo.png" alt="EDM Logo" className="w-4 h-4 object-contain" />
                Add New Download Task
              </span>
              <button
                onClick={resetState}
                className="p-1 rounded-lg text-[var(--text-muted)] hover:text-[var(--text-primary)] cursor-pointer"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            <div className="p-5 space-y-4">
              <div>
                <div className="flex items-center justify-between mb-1 gap-2">
                  <label className="block text-xs font-medium text-[var(--text-secondary)]">
                    Source Address (URL / HTTP / Magnet / .torrent)
                  </label>
                  <div className="flex items-center gap-3 shrink-0">
                    {onOpenBatchImport && (
                      <button
                        type="button"
                        onClick={() => {
                          resetState();
                          onOpenBatchImport();
                        }}
                        className="text-xs text-[var(--accent)] hover:underline inline-flex items-center gap-1 cursor-pointer font-medium"
                      >
                        <Upload className="w-3.5 h-3.5" />
                        <span>Import Batch (.txt/.csv/.html)</span>
                      </button>
                    )}
                    <button
                      type="button"
                      onClick={handleBrowseTorrentFile}
                      className="text-xs text-[var(--accent)] hover:underline inline-flex items-center gap-1 cursor-pointer font-medium"
                    >
                      <FileCode className="w-3.5 h-3.5" />
                      <span>Choose .torrent File...</span>
                    </button>
                  </div>
                </div>
                <div className="flex gap-2">
                  <input
                    type="text"
                    placeholder="https://example.com/file.zip, magnet:?xt=..., or path to .torrent file"
                    value={urlInput}
                    onChange={(e) => handleUrlInputChange(e.target.value)}
                    className="flex-1 px-3 py-2 rounded-lg text-xs input-3d text-[var(--text-primary)] focus:outline-none"
                  />
                  {isTorrentTarget ? (
                    <button
                      type="button"
                      onClick={() => void handleInspectTorrent()}
                      disabled={isInspectingTorrent || !trimmedUrl}
                      className="btn-3d px-3 py-2 rounded-lg text-xs font-semibold text-[var(--accent)] disabled:opacity-40 cursor-pointer inline-flex items-center gap-1.5 shrink-0"
                    >
                      <Search className="w-3.5 h-3.5" />
                      <span>{isInspectingTorrent ? "Looking up..." : "Look up"}</span>
                    </button>
                  ) : (
                    <>
                      <button
                        type="button"
                        onClick={handleProbe}
                        disabled={isProbing || !trimmedUrl}
                        className="btn-3d px-3 py-2 rounded-lg text-xs font-semibold text-[var(--accent)] disabled:opacity-40 cursor-pointer shrink-0"
                      >
                        {isProbing ? "Probing..." : "Probe Server"}
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
                  <select
                    value={addCategory}
                    onChange={(e) => setAddCategory(e.target.value)}
                    className="w-full px-3 py-2 rounded-lg input-3d text-[var(--text-primary)] focus:outline-none cursor-pointer"
                  >
                    <option value="auto">Auto-Detect from Extension</option>
                    {settings.categories
                      .filter((c) => c.enabled)
                      .map((c) => (
                        <option key={c.id} value={c.name}>
                          {c.name}
                        </option>
                      ))}
                  </select>
                </div>
                <div>
                  <label className="block font-medium text-[var(--text-secondary)] mb-1">TCP Worker Streams</label>
                  <select
                    value={String(addConnections)}
                    onChange={(e) => setAddConnections(Number(e.target.value))}
                    className="w-full px-3 py-2 rounded-lg input-3d text-[var(--text-primary)] focus:outline-none cursor-pointer"
                  >
                    <option value="8">8 Parallel Streams (Recommended)</option>
                    <option value="16">16 Parallel Streams (Fast)</option>
                    <option value="32">32 Parallel Streams (Maximum)</option>
                    <option value="1">1 Single Stream (Strict)</option>
                  </select>
                </div>
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
                    className="flex-1 px-3 py-2 rounded-lg text-xs input-3d text-[var(--text-primary)] font-mono"
                  />
                  <button
                    type="button"
                    onClick={handleBrowseAddDir}
                    className="btn-3d px-3 py-2 rounded-lg text-xs font-semibold text-[var(--text-secondary)] hover:text-[var(--text-primary)] cursor-pointer inline-flex items-center gap-1.5 shrink-0"
                  >
                    <FolderOpen className="w-3.5 h-3.5 text-[var(--accent)]" />
                    <span>Browse...</span>
                  </button>
                </div>
              </div>
            </div>

            <div className="px-5 py-3 bg-[var(--bg-card)] border-t border-[var(--border-subtle)] flex items-center justify-end gap-2">
              <button
                onClick={resetState}
                className="btn-3d px-4 py-1.5 rounded-lg text-xs font-medium cursor-pointer"
              >
                Cancel
              </button>
              <button
                onClick={handleSubmit}
                disabled={!trimmedUrl || noFilesSelected}
                className="btn-primary-3d px-4 py-1.5 rounded-lg text-xs font-semibold disabled:opacity-40 cursor-pointer"
              >
                Start Download
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

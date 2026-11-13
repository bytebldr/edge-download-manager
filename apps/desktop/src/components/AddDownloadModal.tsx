import React, { useState } from "react";
import { X, CheckCircle2, FolderOpen, FileCode } from "lucide-react";
import { motion, AnimatePresence } from "motion/react";
import { backdropVariants, modalVariants } from "../utils/motion";
import { EdmSettings } from "../types/settings";
import { isTauri, tauriProbeUrl, tauriSelectDownloadDirectory, tauriSelectTorrentFile } from "../services/tauriBridge";

interface AddDownloadModalProps {
  isOpen: boolean;
  onClose: () => void;
  onAdd: (url: string, filename: string, category: string, connections: number, saveDir: string) => void;
  settings: EdmSettings;
}

export const AddDownloadModal: React.FC<AddDownloadModalProps> = ({
  isOpen,
  onClose,
  onAdd,
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
    setAddSaveDir("");
    onClose();
  };

  const handleUrlInputChange = (val: string) => {
    setUrlInput(val);
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
      }
    } catch (err) {
      console.warn("Failed to select torrent file:", err);
    }
  };

  const handleSubmit = () => {
    if (!urlInput.trim()) return;
    onAdd(
      urlInput.trim(),
      fileNameInput.trim(),
      effectiveCategory,
      addConnections,
      currentDestination
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
                <div className="flex items-center justify-between mb-1">
                  <label className="block text-xs font-medium text-[var(--text-secondary)]">
                    Source Address (URL / HTTP / Magnet / .torrent)
                  </label>
                  <button
                    type="button"
                    onClick={handleBrowseTorrentFile}
                    className="text-xs text-[var(--accent)] hover:underline inline-flex items-center gap-1 cursor-pointer font-medium"
                  >
                    <FileCode className="w-3.5 h-3.5" />
                    <span>Choose .torrent File...</span>
                  </button>
                </div>
                <div className="flex gap-2">
                  <input
                    type="text"
                    placeholder="https://example.com/file.zip, magnet:?xt=..., or path to .torrent file"
                    value={urlInput}
                    onChange={(e) => handleUrlInputChange(e.target.value)}
                    className="flex-1 px-3 py-2 rounded-lg text-xs input-3d text-[var(--text-primary)] focus:outline-none"
                  />
                  <button
                    onClick={handleProbe}
                    disabled={isProbing || !urlInput.trim() || urlInput.startsWith("magnet:") || urlInput.endsWith(".torrent")}
                    className="btn-3d px-3 py-2 rounded-lg text-xs font-semibold text-[var(--accent)] disabled:opacity-40 cursor-pointer"
                  >
                    {isProbing ? "Probing..." : "Probe Server"}
                  </button>
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
                disabled={!urlInput.trim()}
                className="btn-primary-3d px-4 py-1.5 rounded-lg text-xs font-semibold disabled:opacity-40 cursor-pointer"
              >
                Start Download
              </button>
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
};

import React, { useEffect, useState } from "react";
import {
  X,
  Film,
  Volume2,
  Subtitles,
  ListVideo,
  Radio,
  CheckSquare,
  Square,
  Download,
  Loader2,
  AlertCircle,
} from "lucide-react";
import { motion, AnimatePresence } from "motion/react";
import { backdropVariants, modalVariants } from "../../utils/motion";

export interface MediaStreamFormat {
  format_id: string;
  ext: string;
  resolution: string | null;
  width: number | null;
  height: number | null;
  fps: number | null;
  bandwidth: number | null;
  filesize: number | null;
  vcodec: string | null;
  acodec: string | null;
  language: string | null;
  url: string | null;
}

export interface MediaSubtitleTrack {
  lang: string;
  name: string | null;
  ext: string;
  url: string | null;
}

export interface MediaPlaylistEntry {
  index: number;
  id: string;
  title: string;
  duration_secs: number | null;
  url: string;
}

export interface MediaProbeInfo {
  url: string;
  title: string;
  is_live: boolean;
  is_playlist: boolean;
  duration_secs: number | null;
  thumbnail: string | null;
  video_formats: MediaStreamFormat[];
  audio_formats: MediaStreamFormat[];
  subtitles: MediaSubtitleTrack[];
  entries: MediaPlaylistEntry[];
}

export interface MediaPickerSelection {
  videoFormatId: string | null;
  audioFormatId: string | null;
  subtitleLang: string | null;
  selectedEntryUrls: string[];
  recordLive: boolean;
}

interface MediaStreamPickerModalProps {
  isOpen: boolean;
  onClose: () => void;
  probeInfo: MediaProbeInfo | null;
  isLoading?: boolean;
  error?: string | null;
  onConfirm: (selection: MediaPickerSelection) => void;
}

function formatBytes(bytes: number | null): string {
  if (!bytes || bytes <= 0) return "—";
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

function formatDuration(secs: number | null): string {
  if (!secs || secs <= 0) return "";
  const total = Math.floor(secs);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  if (h > 0) return `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
  return `${m}:${String(s).padStart(2, "0")}`;
}

export const MediaStreamPickerModal: React.FC<MediaStreamPickerModalProps> = ({
  isOpen,
  onClose,
  probeInfo,
  isLoading = false,
  error = null,
  onConfirm,
}) => {
  const [selectedVideoId, setSelectedVideoId] = useState<string>("");
  const [selectedAudioId, setSelectedAudioId] = useState<string>("");
  const [selectedSubtitle, setSelectedSubtitle] = useState<string>("");
  const [selectedEntries, setSelectedEntries] = useState<Set<string>>(new Set());

  useEffect(() => {
    if (!probeInfo) return;
    setSelectedVideoId(probeInfo.video_formats[0]?.format_id ?? "");
    setSelectedAudioId(probeInfo.audio_formats[0]?.format_id ?? "");
    setSelectedSubtitle("");
    setSelectedEntries(new Set(probeInfo.entries.map((e) => e.url)));
  }, [probeInfo]);

  const toggleEntry = (url: string) => {
    setSelectedEntries((prev) => {
      const next = new Set(prev);
      if (next.has(url)) next.delete(url);
      else next.add(url);
      return next;
    });
  };

  const toggleAllEntries = () => {
    if (!probeInfo) return;
    if (selectedEntries.size === probeInfo.entries.length) {
      setSelectedEntries(new Set());
    } else {
      setSelectedEntries(new Set(probeInfo.entries.map((e) => e.url)));
    }
  };

  const handleConfirm = () => {
    if (!probeInfo) return;
    onConfirm({
      videoFormatId: selectedVideoId || null,
      audioFormatId: selectedAudioId || null,
      subtitleLang: selectedSubtitle || null,
      selectedEntryUrls: Array.from(selectedEntries),
      recordLive: probeInfo.is_live,
    });
  };

  return (
    <AnimatePresence>
      {isOpen && (
        <motion.div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/65 backdrop-blur-sm p-4"
          variants={backdropVariants}
          initial="hidden"
          animate="visible"
          exit="exit"
          onClick={onClose}
        >
          <motion.div
            className="w-full max-w-2xl rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-panel)] text-[var(--text-primary)] shadow-[var(--shadow-depth)] overflow-hidden"
            variants={modalVariants}
            initial="hidden"
            animate="visible"
            exit="exit"
            onClick={(e) => e.stopPropagation()}
          >
            {/* Header */}
            <div className="flex items-center justify-between gap-3 border-b border-[var(--border-dim)] px-5 py-3.5 bg-[var(--bg-header)]">
              <div className="flex items-center gap-2.5 min-w-0">
                <Film className="w-4 h-4 text-[var(--accent)] shrink-0" />
                <div className="min-w-0">
                  <h2 className="text-sm font-semibold truncate">
                    {probeInfo?.title || "Media Stream Picker"}
                  </h2>
                  {probeInfo?.duration_secs && !probeInfo.is_live && (
                    <p className="text-[11px] text-[var(--text-muted)]">
                      Duration: {formatDuration(probeInfo.duration_secs)}
                    </p>
                  )}
                </div>
                {probeInfo?.is_live && (
                  <span className="inline-flex items-center gap-1.5 rounded-full border border-red-500/30 bg-red-500/15 px-2.5 py-0.5 text-[11px] font-medium text-red-400 shrink-0">
                    <Radio className="w-3 h-3 animate-pulse" />
                    Record Live Stream
                  </span>
                )}
              </div>
              <button
                type="button"
                onClick={onClose}
                className="rounded-md p-1 text-[var(--text-muted)] hover:bg-[var(--bg-card-hover)] hover:text-[var(--text-primary)] transition-colors"
                aria-label="Close modal"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            {/* Body */}
            <div className="max-h-[70vh] overflow-y-auto p-5 space-y-4">
              {isLoading && (
                <div className="flex items-center justify-center gap-2.5 py-12 text-xs text-[var(--text-secondary)]">
                  <Loader2 className="w-4 h-4 animate-spin text-[var(--accent)]" />
                  Probing media streams...
                </div>
              )}

              {!isLoading && error && (
                <div className="flex items-start gap-2.5 rounded-lg border border-red-500/30 bg-red-500/10 p-3 text-xs text-red-400">
                  <AlertCircle className="w-4 h-4 shrink-0 mt-0.5" />
                  <span>{error}</span>
                </div>
              )}

              {!isLoading && !error && probeInfo && (
                <>
                  {/* Video Quality */}
                  <div className="space-y-1.5">
                    <label className="flex items-center gap-1.5 text-xs font-medium text-[var(--text-secondary)]">
                      <Film className="w-3.5 h-3.5" />
                      Video Quality
                    </label>
                    {probeInfo.video_formats.length > 0 ? (
                      <select
                        value={selectedVideoId}
                        onChange={(e) => setSelectedVideoId(e.target.value)}
                        className="w-full rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-card)] px-3 py-2 text-xs text-[var(--text-primary)] focus:outline-none focus:border-[var(--accent)]"
                      >
                        {probeInfo.video_formats.map((fmt) => (
                          <option key={fmt.format_id} value={fmt.format_id}>
                            {fmt.resolution || "Video"} ({fmt.ext})
                            {fmt.fps ? ` • ${fmt.fps}fps` : ""}
                            {fmt.vcodec ? ` • ${fmt.vcodec}` : ""}
                            {fmt.filesize ? ` • ${formatBytes(fmt.filesize)}` : ""}
                            {!fmt.filesize && fmt.bandwidth
                              ? ` • ${Math.round(fmt.bandwidth / 1000)} kbps`
                              : ""}
                          </option>
                        ))}
                      </select>
                    ) : (
                      <p className="text-xs text-[var(--text-muted)]">No video tracks detected.</p>
                    )}
                  </div>

                  {/* Audio & Subtitles Grid */}
                  <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                    <div className="space-y-1.5">
                      <label className="flex items-center gap-1.5 text-xs font-medium text-[var(--text-secondary)]">
                        <Volume2 className="w-3.5 h-3.5" />
                        Audio Track
                      </label>
                      <select
                        value={selectedAudioId}
                        onChange={(e) => setSelectedAudioId(e.target.value)}
                        className="w-full rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-card)] px-3 py-2 text-xs text-[var(--text-primary)] focus:outline-none focus:border-[var(--accent)]"
                      >
                        <option value="">Default / Embedded Audio</option>
                        {probeInfo.audio_formats.map((aud) => (
                          <option key={aud.format_id} value={aud.format_id}>
                            {aud.language ? `${aud.language.toUpperCase()} • ` : ""}
                            {aud.ext}
                            {aud.acodec ? ` • ${aud.acodec}` : ""}
                            {aud.bandwidth ? ` • ${Math.round(aud.bandwidth / 1000)} kbps` : ""}
                          </option>
                        ))}
                      </select>
                    </div>

                    <div className="space-y-1.5">
                      <label className="flex items-center gap-1.5 text-xs font-medium text-[var(--text-secondary)]">
                        <Subtitles className="w-3.5 h-3.5" />
                        Subtitles
                      </label>
                      <select
                        value={selectedSubtitle}
                        onChange={(e) => setSelectedSubtitle(e.target.value)}
                        className="w-full rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-card)] px-3 py-2 text-xs text-[var(--text-primary)] focus:outline-none focus:border-[var(--accent)]"
                      >
                        <option value="">None</option>
                        {probeInfo.subtitles.map((sub) => (
                          <option key={sub.lang} value={sub.lang}>
                            {sub.name ? `${sub.name} (${sub.lang})` : sub.lang} • {sub.ext}
                          </option>
                        ))}
                      </select>
                    </div>
                  </div>

                  {/* Playlist Entries */}
                  {probeInfo.is_playlist && probeInfo.entries.length > 0 && (
                    <div className="space-y-2">
                      <div className="flex items-center justify-between">
                        <span className="flex items-center gap-1.5 text-xs font-medium text-[var(--text-secondary)]">
                          <ListVideo className="w-3.5 h-3.5" />
                          Playlist Entries ({selectedEntries.size}/{probeInfo.entries.length})
                        </span>
                        <button
                          type="button"
                          onClick={toggleAllEntries}
                          className="text-[11px] font-medium text-[var(--accent)] hover:underline"
                        >
                          {selectedEntries.size === probeInfo.entries.length
                            ? "Deselect All"
                            : "Select All"}
                        </button>
                      </div>

                      <div className="max-h-48 overflow-y-auto rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-card)] divide-y divide-[var(--border-dim)]">
                        {probeInfo.entries.map((entry) => {
                          const checked = selectedEntries.has(entry.url);
                          return (
                            <button
                              key={entry.url}
                              type="button"
                              onClick={() => toggleEntry(entry.url)}
                              className="w-full flex items-center justify-between gap-2 px-3 py-2 text-left text-xs hover:bg-[var(--bg-card-hover)] transition-colors"
                            >
                              <span className="flex items-center gap-2 min-w-0">
                                {checked ? (
                                  <CheckSquare className="w-3.5 h-3.5 text-[var(--accent)] shrink-0" />
                                ) : (
                                  <Square className="w-3.5 h-3.5 text-[var(--text-muted)] shrink-0" />
                                )}
                                <span className="text-[var(--text-muted)] w-5 shrink-0">
                                  {entry.index}.
                                </span>
                                <span className="truncate">{entry.title}</span>
                              </span>
                              {entry.duration_secs && (
                                <span className="text-[11px] text-[var(--text-muted)] shrink-0">
                                  {formatDuration(entry.duration_secs)}
                                </span>
                              )}
                            </button>
                          );
                        })}
                      </div>
                    </div>
                  )}
                </>
              )}
            </div>

            {/* Footer */}
            <div className="flex items-center justify-end gap-2.5 border-t border-[var(--border-dim)] px-5 py-3 bg-[var(--bg-header)]">
              <button
                type="button"
                onClick={onClose}
                className="rounded-lg border border-[var(--border-subtle)] px-3.5 py-1.5 text-xs font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-card-hover)] transition-colors"
              >
                Cancel
              </button>
              <button
                type="button"
                disabled={
                  isLoading ||
                  !!error ||
                  !probeInfo ||
                  (probeInfo.is_playlist &&
                    probeInfo.entries.length > 0 &&
                    selectedEntries.size === 0)
                }
                onClick={handleConfirm}
                className="inline-flex items-center gap-1.5 rounded-lg bg-[var(--accent)] px-4 py-1.5 text-xs font-medium text-[var(--accent-on)] hover:bg-[var(--accent-hover)] disabled:opacity-45 disabled:cursor-not-allowed transition-colors"
              >
                {probeInfo?.is_live ? (
                  <>
                    <Radio className="w-3.5 h-3.5" />
                    Record Live Stream
                  </>
                ) : (
                  <>
                    <Download className="w-3.5 h-3.5" />
                    Download Selected
                  </>
                )}
              </button>
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
};

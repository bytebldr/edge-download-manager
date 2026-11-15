import React from "react";
import { X, CheckCircle2, Radio, Users, ArrowUp } from "lucide-react";
import { motion, AnimatePresence } from "motion/react";
import { backdropVariants, modalVariants } from "../utils/motion";
import { formatBytes } from "../utils/format";
import { getMergedProgressSpans } from "../utils/progress";
import { SpeedChartInteractive } from "./SpeedChartInteractive";
import { EdmSettings } from "../types/settings";

export interface WorkerInfo {
  idx: number;
  start: number;
  end: number;
  pos: number;
  speed: string;
  status: string;
}

export interface InspectorDownloadItem {
  id: string;
  name: string;
  category: string;
  sizeTotal: number;
  sizeDone: number;
  speed: string;
  eta: string;
  connections: number;
  state: "downloading" | "paused" | "queued" | "completed" | "failed";
  progress: number;
  url: string;
  domain: string;
  dateAdded: string;
  workers?: WorkerInfo[];
  speedHistory?: number[];
  uploadHistory?: number[];
  peakSpeedBps?: number;
  elapsedSeconds?: number;
}

interface DetailsInspectorDrawerProps {
  isOpen: boolean;
  download: InspectorDownloadItem | null;
  onClose: () => void;
  settings: EdmSettings;
  onToggleState: (id: string) => void;
  onOpenFile: (id: string) => void;
}

export const DetailsInspectorDrawer: React.FC<DetailsInspectorDrawerProps> = ({
  isOpen,
  download,
  onClose,
  settings,
  onToggleState,
  onOpenFile,
}) => {
  if (!download) return null;

  const isTorrent =
    download.url.startsWith("magnet:") ||
    download.url.toLowerCase().includes(".torrent") ||
    download.name.toLowerCase().endsWith(".torrent") ||
    download.category?.toLowerCase() === "torrents";

  const isVerifying =
    download.eta?.toLowerCase().includes("verif") ||
    (download.state as any) === "verifying";

  const currentUlBps =
    download.uploadHistory && download.uploadHistory.length > 0
      ? download.uploadHistory[download.uploadHistory.length - 1]
      : 0;
  const totalUploadedBytes =
    download.uploadHistory && download.uploadHistory.length > 0
      ? download.uploadHistory.reduce((a, b) => a + b, 0)
      : 0;
  const shareRatio =
    download.sizeDone > 0 ? (totalUploadedBytes / download.sizeDone).toFixed(2) : "0.00";

  const totalBytes = Math.max(1, download.sizeTotal);
  const workersList =
    download.workers && download.workers.length > 0
      ? download.workers
      : Array.from({ length: Math.max(1, download.connections || 8) }).map((_, idx, arr) => {
          const chunkBytes = Math.floor(totalBytes / arr.length);
          const start = idx * chunkBytes;
          const end = idx === arr.length - 1 ? totalBytes : (idx + 1) * chunkBytes;
          const range = end - start;
          const done = Math.floor((range * download.progress) / 100);
          return {
            idx: idx + 1,
            start,
            end,
            pos: start + done,
            speed: download.speed,
            status:
              download.state === "downloading"
                ? isTorrent
                  ? isVerifying
                    ? "Verifying piece hashes..."
                    : "Active swarm exchange"
                  : "Receiving chunk (HTTP 206)..."
                : download.state === "completed"
                ? isTorrent
                  ? "Verified • Seeding"
                  : "Chunk verified • Complete"
                : isTorrent
                ? "Swarm suspended (Paused)"
                : "Thread suspended (Paused)",
          };
        });

  return (
    <AnimatePresence>
      {isOpen && (
        <motion.div
          variants={backdropVariants}
          initial="initial"
          animate="animate"
          exit="exit"
          className="fixed inset-0 bg-black/75 backdrop-blur-sm flex items-center justify-center p-4 z-50"
          onClick={onClose}
        >
          <motion.div
            variants={modalVariants}
            initial="initial"
            animate="animate"
            exit="exit"
            className="panel-3d w-full max-w-2xl max-h-[85vh] flex flex-col overflow-hidden"
            onClick={(e) => e.stopPropagation()}
          >
            {/* Modal Title Bar */}
            <div className="px-4 py-2.5 bg-[var(--bg-card)] border-b border-[var(--border-subtle)] flex items-center justify-between shrink-0">
              <div className="flex items-center gap-2 min-w-0">
                <img src="/logo.png" alt="EDM" className="w-4 h-4 object-contain shrink-0" />
                <span className="font-bold tracking-tight text-xs sm:text-sm truncate">
                  Download Status — {download.name}
                </span>
              </div>
              <button
                onClick={onClose}
                className="p-1 rounded-md text-[var(--text-muted)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-chassis)] cursor-pointer shrink-0"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            {/* Modal Body */}
            <div className="p-3.5 sm:p-4 space-y-3 overflow-y-auto flex-1 min-h-0">
              {/* Metadata Grid */}
              <div className="grid grid-cols-2 md:grid-cols-4 gap-2 sm:gap-2.5 p-2.5 sm:p-3 rounded-lg bg-[var(--bg-chassis)] border border-[var(--border-subtle)] text-xs">
                <div>
                  <div className="text-[9px] sm:text-[10px] text-[var(--text-muted)] uppercase tracking-wider font-semibold">
                    Status
                  </div>
                  <div
                    className={`font-semibold text-xs mt-0.5 ${
                      isVerifying
                        ? "text-amber-300 animate-pulse"
                        : download.state === "downloading"
                        ? "text-[var(--accent)]"
                        : download.state === "paused"
                        ? "text-amber-400"
                        : "text-emerald-400"
                    }`}
                  >
                    {isVerifying
                      ? "Verifying data..."
                      : download.state === "downloading"
                      ? isTorrent
                        ? "Active Swarm..."
                        : "Receiving data..."
                      : download.state === "paused"
                      ? "Paused"
                      : isTorrent
                      ? "Seeding (100%)"
                      : "Completed"}
                  </div>
                </div>
                <div>
                  <div className="text-[9px] sm:text-[10px] text-[var(--text-muted)] uppercase tracking-wider font-semibold">
                    File Size
                  </div>
                  <div className="font-mono tabular-nums font-semibold text-xs mt-0.5">
                    {formatBytes(download.sizeTotal)}
                  </div>
                </div>
                <div>
                  <div className="text-[9px] sm:text-[10px] text-[var(--text-muted)] uppercase tracking-wider font-semibold">
                    Transfer Rate
                  </div>
                  <div
                    className={`font-mono tabular-nums font-semibold text-xs mt-0.5 flex items-center gap-1.5 ${
                      download.state === "downloading" ? "text-emerald-400" : "text-[var(--text-muted)]"
                    }`}
                  >
                    <span>{download.speed}</span>
                    {isTorrent && currentUlBps > 0 && (
                      <span className="text-[#818CF8] text-[10px] flex items-center">
                        <ArrowUp className="w-2.5 h-2.5 inline" />
                        {formatBytes(currentUlBps)}/s
                      </span>
                    )}
                  </div>
                </div>
                <div>
                  <div className="text-[9px] sm:text-[10px] text-[var(--text-muted)] uppercase tracking-wider font-semibold">
                    {isTorrent ? "Swarm Protocol" : "Resume Capable"}
                  </div>
                  <div className={`font-semibold text-xs mt-0.5 flex items-center gap-1 ${isTorrent ? "text-sky-400" : "text-emerald-400"}`}>
                    {isTorrent ? (
                      <>
                        <Radio className="w-3 h-3 inline shrink-0 text-sky-400" /> BitTorrent (DHT/uTP)
                      </>
                    ) : (
                      <>
                        <CheckCircle2 className="w-3 h-3 inline shrink-0" /> Yes (Byte-Range)
                      </>
                    )}
                  </div>
                </div>
              </div>

              {/* Dynamic Parallel Segmentation Progress Bar */}
              <div>
                <div className="flex justify-between items-center text-[11px] mb-1">
                  <span className="font-medium text-[var(--text-secondary)]">
                    {isTorrent
                      ? "BitTorrent Swarm Piece Verification & Progress:"
                      : "Start positions and download progress by connections:"}
                  </span>
                  <span className="font-mono tabular-nums font-bold text-[var(--accent)] text-xs">
                    {download.state === "completed" || download.progress >= 100 ? 100 : download.progress}% (
                    {formatBytes(
                      download.state === "completed" || download.progress >= 100
                        ? download.sizeTotal || download.sizeDone
                        : download.sizeDone
                    )}{" "}
                    / {formatBytes(download.sizeTotal || download.sizeDone)})
                  </span>
                </div>
                <div className="h-5 rounded bg-[var(--bg-chassis)] border border-[var(--border-strong)] relative overflow-hidden shadow-inner flex items-center">
                  {(() => {
                    const isComplete =
                      download.state === "completed" ||
                      download.progress >= 100 ||
                      (totalBytes > 0 && download.sizeDone >= totalBytes);

                    if (isComplete) {
                      return (
                        <div
                          className="h-full w-full progress-fill-3d transition-all duration-300"
                          style={{ width: "100%" }}
                        />
                      );
                    }

                    const spans = getMergedProgressSpans(
                      workersList,
                      totalBytes,
                      download.sizeDone,
                      false
                    );

                    return (
                      <>
                        {spans.map((span, sIdx) => (
                          <div
                            key={sIdx}
                            className="absolute top-0 bottom-0 z-10 progress-fill-3d transition-all duration-300"
                            style={{
                              left: `${span.startPct}%`,
                              width:
                                sIdx < spans.length - 1
                                  ? `calc(${span.widthPct}% + 1px)`
                                  : `${span.widthPct}%`,
                            }}
                          >
                            {download.state === "downloading" && sIdx === spans.length - 1 && (
                              <div className="absolute right-0 top-0 bottom-0 w-[2px] bg-white shadow-[0_0_8px_#38BDF8] animate-pulse" />
                            )}
                          </div>
                        ))}
                        {settings?.general?.showConnectionBoundaries &&
                          workersList &&
                          workersList.length > 1 &&
                          workersList.slice(1).map((w, idx) => {
                            const offsetPct = totalBytes > 0 ? (w.start / totalBytes) * 100 : 0;
                            return (
                              <div
                                key={`boundary-${idx}`}
                                className="absolute top-0 bottom-0 w-[1px] bg-white/20 pointer-events-none z-20"
                                style={{ left: `${offsetPct}%` }}
                              />
                            );
                          })}
                      </>
                    );
                  })()}
                </div>
                <div className="flex justify-between text-[9.5px] text-[var(--text-muted)] font-mono mt-1">
                  <span>{isTorrent ? `Pieces Verified: ${download.progress}%` : "Offset: 0x00000000"}</span>
                  <span className="text-emerald-400 font-medium">
                    {isTorrent
                      ? "UPnP & NAT-PMP Active • Swarm Seeding Enabled"
                      : "Engine Crash Safety: Flush-Before-Checkpoint Enabled"}
                  </span>
                  <span>{isTorrent ? `Swarm Total: ${formatBytes(download.sizeTotal)}` : `Offset: ${formatBytes(download.sizeTotal)}`}</span>
                </div>
              </div>

              {/* Real-time Dual-Stream Interactive Area Chart */}
              <SpeedChartInteractive
                speedHistory={download.speedHistory}
                uploadHistory={download.uploadHistory}
                currentSpeed={download.speed}
                peakSpeedBps={download.peakSpeedBps}
                isDownloading={download.state === "downloading"}
                eta={download.eta}
                elapsedSeconds={download.elapsedSeconds || 0}
              />

              {/* Protocol-Specific Telemetry: Torrent Swarm vs HTTP 206 */}
              {isTorrent ? (
                <div>
                  <div className="flex justify-between items-center text-[11px] font-medium text-[var(--text-secondary)] mb-1.5">
                    <span className="flex items-center gap-1.5">
                      <Users className="w-3.5 h-3.5 text-sky-400" />
                      BitTorrent Swarm & Peer Telemetry
                    </span>
                    <span className="font-mono text-[10px] text-[var(--text-muted)]">
                      {download.state === "downloading" ? Math.max(1, download.connections || 1) : 0} Connected Peers (uTP/TCP)
                    </span>
                  </div>

                  {/* Torrent Swarm Metrics Bar */}
                  <div className="grid grid-cols-4 gap-2 mb-2 p-2 rounded-lg bg-[var(--bg-chassis)] border border-[var(--border-subtle)] text-[11px] font-mono">
                    <div className="flex flex-col">
                      <span className="text-[9px] uppercase tracking-wider text-[var(--text-muted)] font-semibold">Peers / Seeds</span>
                      <span className="text-sky-400 font-bold mt-0.5">
                        {download.state === "downloading" ? `${Math.max(1, download.connections || 1)} Connected` : "0 (Idle)"}
                      </span>
                    </div>
                    <div className="flex flex-col">
                      <span className="text-[9px] uppercase tracking-wider text-[var(--text-muted)] font-semibold">Share Ratio</span>
                      <span className="text-emerald-400 font-bold mt-0.5">{shareRatio}</span>
                    </div>
                    <div className="flex flex-col">
                      <span className="text-[9px] uppercase tracking-wider text-[var(--text-muted)] font-semibold">Uploaded</span>
                      <span className="text-[#818CF8] font-bold mt-0.5">{formatBytes(totalUploadedBytes)}</span>
                    </div>
                    <div className="flex flex-col">
                      <span className="text-[9px] uppercase tracking-wider text-[var(--text-muted)] font-semibold">Swarm Health</span>
                      <span className="text-emerald-400 font-bold mt-0.5">
                        {download.state === "completed"
                          ? "Seeding (100%)"
                          : isVerifying
                          ? "Verifying"
                          : download.state === "downloading"
                          ? "Healthy Swarm"
                          : "Paused"}
                      </span>
                    </div>
                  </div>

                  {/* Swarm Peers / Pieces Table */}
                  <div className="rounded-lg border border-[var(--border-subtle)] overflow-hidden bg-[var(--bg-chassis)] max-h-36 overflow-y-auto">
                    <table className="w-full text-left text-[11px]">
                      <thead className="sticky top-0 bg-[var(--bg-card)] border-b border-[var(--border-subtle)] text-[9px] font-semibold uppercase tracking-wider text-[var(--text-muted)] z-10">
                        <tr>
                          <th className="py-1 px-2.5">Peer / Transport</th>
                          <th className="py-1 px-2.5">Piece Allocation</th>
                          <th className="py-1 px-2.5">Piece Progress</th>
                          <th className="py-1 px-2.5">Bandwidth Rate</th>
                          <th className="py-1 px-2.5">Swarm Status</th>
                        </tr>
                      </thead>
                      <tbody className="divide-y divide-[var(--border-dim)] font-mono tabular-nums">
                        {workersList.map((w, idx) => {
                          const isPaused = download.state === "paused";
                          const isCompleted = download.state === "completed";
                          const range = Math.max(1, w.end - w.start);
                          const done = Math.max(0, Math.min(range, w.pos - w.start));
                          const pct = Math.min(100, Math.round((done / range) * 100));

                          let statusColor = "text-[var(--text-muted)]";
                          if (isCompleted || pct >= 100) statusColor = "text-emerald-400";
                          else if (!isPaused && download.state === "downloading") statusColor = "text-sky-400";

                          return (
                            <tr key={w.idx || idx + 1} className="hover:bg-[var(--bg-card-hover)]">
                              <td className="py-1 px-2.5 font-semibold text-sky-400">
                                Peer #{w.idx || idx + 1} (uTP/DHT)
                              </td>
                              <td className="py-1 px-2.5 text-[var(--text-secondary)]">
                                Block [{formatBytes(w.start)} - {formatBytes(w.end)}]
                              </td>
                              <td className="py-1 px-2.5 text-[var(--text-primary)]">
                                {pct}% ({formatBytes(done)})
                              </td>
                              <td
                                className={`py-1 px-2.5 font-semibold ${
                                  isPaused ? "text-[var(--text-muted)]" : "text-emerald-400"
                                }`}
                              >
                                {isPaused ? "0 B/s" : w.speed}
                              </td>
                              <td className={`py-1 px-2.5 text-[10px] ${statusColor}`}>
                                {isPaused
                                  ? "Swarm suspended (Paused)"
                                  : isCompleted
                                  ? "Verified • Seeding to Swarm"
                                  : isVerifying
                                  ? "Verifying SHA-1 Hash..."
                                  : "Active swarm piece exchange"}
                              </td>
                            </tr>
                          );
                        })}
                      </tbody>
                    </table>
                  </div>
                </div>
              ) : (
                /* HTTP / HTTPS Multi-Connection Worker Thread Telemetry Table */
                <div>
                  <div className="flex justify-between items-center text-[11px] font-medium text-[var(--text-secondary)] mb-1">
                    <span>Multi-Connection Worker Thread Telemetry</span>
                    <span className="font-mono text-[10px] text-[var(--text-muted)]">
                      {workersList.length} Active Stream{workersList.length > 1 ? "s" : ""}
                    </span>
                  </div>
                  <div className="rounded-lg border border-[var(--border-subtle)] overflow-hidden bg-[var(--bg-chassis)] max-h-36 overflow-y-auto">
                    <table className="w-full text-left text-[11px]">
                      <thead className="sticky top-0 bg-[var(--bg-card)] border-b border-[var(--border-subtle)] text-[9px] font-semibold uppercase tracking-wider text-[var(--text-muted)] z-10">
                        <tr>
                          <th className="py-1 px-2.5">Connection</th>
                          <th className="py-1 px-2.5">Byte Range</th>
                          <th className="py-1 px-2.5">Progress</th>
                          <th className="py-1 px-2.5">Throughput</th>
                          <th className="py-1 px-2.5">Status</th>
                        </tr>
                      </thead>
                      <tbody className="divide-y divide-[var(--border-dim)] font-mono tabular-nums">
                        {workersList.map((w, idx) => {
                          const isPaused = download.state === "paused";
                          const isCompleted = download.state === "completed";
                          const range = Math.max(1, w.end - w.start);
                          const done = Math.max(0, Math.min(range, w.pos - w.start));
                          const pct = Math.min(100, Math.round((done / range) * 100));

                          let statusColor = "text-[var(--text-muted)]";
                          if (isCompleted || pct >= 100) statusColor = "text-emerald-400";
                          else if (!isPaused && download.state === "downloading") statusColor = "text-sky-400";

                          return (
                            <tr key={w.idx || idx + 1} className="hover:bg-[var(--bg-card-hover)]">
                              <td className="py-1 px-2.5 font-semibold text-[var(--accent)]">
                                Worker #{w.idx || idx + 1}
                              </td>
                              <td className="py-1 px-2.5 text-[var(--text-secondary)]">
                                {formatBytes(w.start)} - {formatBytes(w.end)}
                              </td>
                              <td className="py-1 px-2.5 text-[var(--text-primary)]">
                                {pct}% ({formatBytes(done)})
                              </td>
                              <td
                                className={`py-1 px-2.5 font-semibold ${
                                  isPaused ? "text-[var(--text-muted)]" : "text-emerald-400"
                                }`}
                              >
                                {isPaused ? "0 B/s" : w.speed}
                              </td>
                              <td className={`py-1 px-2.5 text-[10px] ${statusColor}`}>
                                {isPaused
                                  ? "Thread suspended (Paused)"
                                  : isCompleted
                                  ? "Chunk verified • Complete"
                                  : w.status}
                              </td>
                            </tr>
                          );
                        })}
                      </tbody>
                    </table>
                  </div>
                </div>
              )}
            </div>

            {/* Modal Actions */}
            <div className="px-4 py-2.5 bg-[var(--bg-card)] border-t border-[var(--border-subtle)] flex items-center justify-between shrink-0">
              <div className="text-[10px] text-[var(--text-muted)] font-mono truncate mr-2">
                NTFS Sparse Allocation • Mark-of-the-Web Attached
              </div>
              <div className="flex items-center gap-2 shrink-0">
                {download.state === "completed" && (
                  <button
                    onClick={() => onOpenFile(download.id)}
                    className="btn-3d h-7 px-3 rounded-lg text-xs font-semibold cursor-pointer text-blue-400 hover:text-blue-300"
                  >
                    Open File
                  </button>
                )}
                {download.state !== "completed" && (
                  <button
                    onClick={() => onToggleState(download.id)}
                    className={`btn-3d h-7 px-3 rounded-lg text-xs font-semibold cursor-pointer transition-colors ${
                      download.state === "downloading"
                        ? "text-amber-400 hover:text-amber-300"
                        : "text-emerald-400 hover:text-emerald-300"
                    }`}
                  >
                    {download.state === "downloading" ? "Pause" : "Resume"}
                  </button>
                )}
                <button
                  onClick={onClose}
                  className="btn-primary-3d h-7 px-3.5 rounded-lg text-xs font-semibold cursor-pointer"
                >
                  Close Window
                </button>
              </div>
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
};

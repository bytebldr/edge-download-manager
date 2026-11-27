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
  uploadedBytes?: number;
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
    download.uploadedBytes ??
    (download.uploadHistory && download.uploadHistory.length > 0
      ? download.uploadHistory.reduce((a, b) => a + b, 0)
      : 0);
  const shareBase = download.sizeDone > 0 ? download.sizeDone : Math.max(1, download.sizeTotal);
  const shareRatio = (totalUploadedBytes / shareBase).toFixed(2);

  const totalBytes = Math.max(1, download.sizeTotal);
  const isQueued = download.state === "queued";
  const isPaused = download.state === "paused";
  const isFailed = download.state === "failed";
  const isCompleted = download.state === "completed";
  const activeStreamCount =
    download.state === "downloading" ? Math.max(1, download.connections || 8) : 0;

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
            speed: download.state === "downloading" ? download.speed : "0 B/s",
            status:
              download.state === "downloading"
                ? isTorrent
                  ? isVerifying
                    ? "Verifying piece hashes"
                    : "Active swarm exchange"
                  : "Receiving chunk (HTTP 206)"
                : isCompleted
                  ? isTorrent
                    ? "Verified • Seeding"
                    : "Chunk verified • Complete"
                  : isQueued
                    ? "Queued (Waiting for slot)"
                    : isFailed
                      ? "Transfer interrupted"
                      : isTorrent
                        ? "Swarm suspended (Paused)"
                        : "Thread suspended (Paused)",
          };
        });

  const statusMeta = isVerifying
    ? { label: "Verifying data", dot: "bg-amber-400", text: "text-amber-300" }
    : download.state === "downloading"
      ? { label: isTorrent ? "Active swarm" : "Downloading", dot: "bg-[var(--accent)]", text: "text-[var(--text-primary)]" }
      : isPaused
        ? { label: "Paused", dot: "bg-amber-400", text: "text-[var(--text-primary)]" }
        : isQueued
          ? { label: "Queued", dot: "bg-zinc-500", text: "text-[var(--text-secondary)]" }
          : isFailed
            ? { label: "Failed", dot: "bg-rose-400", text: "text-rose-400" }
            : { label: isTorrent ? "Seeding" : "Completed", dot: "bg-emerald-400", text: "text-emerald-400" };

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
            role="dialog"
            aria-modal="true"
            aria-label={`Download Status — ${download.name}`}
            className="w-full max-w-2xl max-h-[85vh] rounded-xl bg-[var(--bg-panel)] border border-[var(--border-subtle)] shadow-[0_24px_48px_-12px_rgba(0,0,0,0.7)] flex flex-col overflow-hidden"
            onClick={(e) => e.stopPropagation()}
          >
            {/* Modal Title Bar */}
            <div className="px-4 py-3 border-b border-[var(--border-dim)] flex items-center justify-between shrink-0">
              <div className="flex items-center gap-2.5 min-w-0">
                <img src="/logo.png" alt="EDM" className="w-4 h-4 object-contain shrink-0 pointer-events-none" />
                <span className="font-semibold tracking-tight text-xs sm:text-sm text-[var(--text-primary)] truncate">
                  Download Status — {download.name}
                </span>
              </div>
              <button
                onClick={onClose}
                aria-label="Close Download Status"
                className="w-7 h-7 rounded-lg flex items-center justify-center text-[var(--text-muted)] hover:text-[var(--text-primary)] hover:bg-white/[0.06] transition-colors cursor-pointer shrink-0"
              >
                <X className="w-4 h-4 stroke-[1.75]" />
              </button>
            </div>

            {/* Modal Body */}
            <div className="p-4 space-y-4 overflow-y-auto flex-1 min-h-0">
              {/* Metadata Strip (Hairline divider, no boxed card soup) */}
              <div className="grid grid-cols-2 md:grid-cols-4 gap-3 pb-3.5 border-b border-[var(--border-dim)] text-xs">
                <div>
                  <div className="text-[11px] text-[var(--text-muted)] font-medium">
                    Status
                  </div>
                  <div className={`font-medium text-xs mt-1 flex items-center gap-1.5 ${statusMeta.text}`}>
                    <span className={`w-1.5 h-1.5 rounded-full shrink-0 ${statusMeta.dot}`} />
                    <span>{statusMeta.label}</span>
                  </div>
                </div>
                <div>
                  <div className="text-[11px] text-[var(--text-muted)] font-medium">
                    File size
                  </div>
                  <div className="tabular-nums font-medium text-xs text-[var(--text-primary)] mt-1">
                    {formatBytes(download.sizeTotal)}
                  </div>
                </div>
                <div>
                  <div className="text-[11px] text-[var(--text-muted)] font-medium">
                    Transfer rate
                  </div>
                  <div
                    className={`tabular-nums font-medium text-xs mt-1 flex items-center gap-1.5 ${
                      download.state === "downloading" ? "text-[var(--accent)]" : "text-[var(--text-secondary)]"
                    }`}
                  >
                    <span>{download.state === "downloading" ? download.speed : "0 B/s"}</span>
                    {isTorrent && currentUlBps > 0 && (
                      <span className="text-[#818CF8] text-[11px] flex items-center">
                        <ArrowUp className="w-3 h-3 inline" />
                        {formatBytes(currentUlBps)}/s
                      </span>
                    )}
                  </div>
                </div>
                <div>
                  <div className="text-[11px] text-[var(--text-muted)] font-medium">
                    {isTorrent ? "Swarm protocol" : "Resume capable"}
                  </div>
                  <div className="font-medium text-xs text-[var(--text-primary)] mt-1 flex items-center gap-1.5">
                    {isTorrent ? (
                      <>
                        <Radio className="w-3.5 h-3.5 shrink-0 text-sky-400 stroke-[1.75]" /> BitTorrent (DHT/uTP)
                      </>
                    ) : (
                      <>
                        <CheckCircle2 className="w-3.5 h-3.5 shrink-0 text-emerald-400 stroke-[1.75]" /> Byte-Range
                      </>
                    )}
                  </div>
                </div>
              </div>

              {/* Dynamic Parallel Segmentation Progress Bar */}
              <div>
                <div className="flex justify-between items-center text-xs mb-1.5">
                  <span className="font-medium text-[var(--text-secondary)]">
                    {isTorrent
                      ? "Swarm piece verification & progress"
                      : "Segment progress by connections"}
                  </span>
                  <span className="tabular-nums font-medium text-[var(--text-primary)]">
                    {isCompleted || download.progress >= 100 ? 100 : download.progress}%{" "}
                    <span className="text-[var(--text-muted)]">
                      ({formatBytes(isCompleted || download.progress >= 100 ? download.sizeTotal || download.sizeDone : download.sizeDone)} / {formatBytes(download.sizeTotal || download.sizeDone)})
                    </span>
                  </span>
                </div>
                <div className="h-2 rounded-full bg-white/[0.06] relative overflow-hidden flex items-center">
                  {(() => {
                    const isComplete =
                      isCompleted ||
                      download.progress >= 100 ||
                      (totalBytes > 0 && download.sizeDone >= totalBytes);

                    if (isComplete) {
                      return (
                        <div
                          className="h-full w-full rounded-full bg-emerald-500 transition-[width] duration-300"
                          style={{ width: "100%" }}
                        />
                      );
                    }

                    if (isQueued && download.progress === 0) {
                      return null;
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
                            className={`absolute top-0 bottom-0 z-10 rounded-full transition-[width] duration-300 ${
                              isPaused ? "bg-amber-500/85" : isFailed ? "bg-rose-500/85" : "bg-[var(--accent)]"
                            }`}
                            style={{
                              left: `${span.startPct}%`,
                              width:
                                sIdx < spans.length - 1
                                  ? `calc(${span.widthPct}% + 1px)`
                                  : `${span.widthPct}%`,
                            }}
                          />
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
                <div className="flex justify-between text-[11px] text-[var(--text-muted)] tabular-nums mt-1.5">
                  <span className="font-mono">{isTorrent ? `Pieces verified: ${download.progress}%` : "0x00000000"}</span>
                  <span>
                    {isTorrent
                      ? "UPnP & NAT-PMP active"
                      : "Crash-safe checkpointing"}
                  </span>
                  <span>{isTorrent ? `Total: ${formatBytes(download.sizeTotal)}` : formatBytes(download.sizeTotal)}</span>
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
                  <div className="flex justify-between items-center text-xs font-medium text-[var(--text-secondary)] mb-2">
                    <span>Worker thread telemetry</span>
                    <span className="tabular-nums text-[11px] text-[var(--text-muted)]">
                      {activeStreamCount} active stream{activeStreamCount === 1 ? "" : "s"}
                    </span>
                  </div>
                  <div className="border-t border-[var(--border-dim)] overflow-hidden max-h-36 overflow-y-auto">
                    <table className="w-full text-left text-[11px]">
                      <thead className="sticky top-0 bg-[var(--bg-panel)] border-b border-[var(--border-dim)] text-[11px] font-medium text-[var(--text-muted)] z-10">
                        <tr>
                          <th className="py-1.5 px-2">Connection</th>
                          <th className="py-1.5 px-2">Byte range</th>
                          <th className="py-1.5 px-2">Progress</th>
                          <th className="py-1.5 px-2">Throughput</th>
                          <th className="py-1.5 px-2">Status</th>
                        </tr>
                      </thead>
                      <tbody className="divide-y divide-[var(--border-dim)] tabular-nums">
                        {workersList.map((w, idx) => {
                          const wPaused = download.state === "paused";
                          const wCompleted = download.state === "completed";
                          const range = Math.max(1, w.end - w.start);
                          const done = Math.max(0, Math.min(range, w.pos - w.start));
                          const pct = Math.min(100, Math.round((done / range) * 100));

                          return (
                            <tr key={w.idx || idx + 1} className="hover:bg-white/[0.025] transition-colors">
                              <td className="py-1.5 px-2 font-medium text-[var(--text-primary)]">
                                Worker #{w.idx || idx + 1}
                              </td>
                              <td className="py-1.5 px-2 text-[var(--text-secondary)]">
                                {formatBytes(w.start)} – {formatBytes(w.end)}
                              </td>
                              <td className="py-1.5 px-2 text-[var(--text-primary)]">
                                {pct}% ({formatBytes(done)})
                              </td>
                              <td
                                className={`py-1.5 px-2 font-medium ${
                                  download.state === "downloading" ? "text-[var(--accent)]" : "text-[var(--text-muted)]"
                                }`}
                              >
                                {download.state === "downloading" ? w.speed : "0 B/s"}
                              </td>
                              <td className="py-1.5 px-2 text-[11px] text-[var(--text-secondary)]">
                                {wPaused
                                  ? "Paused"
                                  : wCompleted
                                    ? "Complete"
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
            <div className="px-4 py-3 border-t border-[var(--border-dim)] flex items-center justify-between shrink-0">
              <div className="text-[11px] text-[var(--text-muted)] truncate mr-2">
                NTFS sparse allocation • Mark-of-the-Web attached
              </div>
              <div className="flex items-center gap-2 shrink-0">
                {download.state === "completed" && (
                  <button
                    onClick={() => onOpenFile(download.id)}
                    className="h-8 px-3 rounded-lg bg-white/[0.04] border border-[var(--border-subtle)] text-xs font-medium text-[var(--text-primary)] hover:bg-white/[0.08] cursor-pointer transition-colors"
                  >
                    Open file
                  </button>
                )}
                {download.state !== "completed" && (
                  <button
                    onClick={() => onToggleState(download.id)}
                    className="h-8 px-3 rounded-lg bg-white/[0.04] border border-[var(--border-subtle)] text-xs font-medium text-[var(--text-primary)] hover:bg-white/[0.08] cursor-pointer transition-colors"
                  >
                    {download.state === "downloading" ? "Pause" : isQueued ? "Start" : "Resume"}
                  </button>
                )}
                <button
                  onClick={onClose}
                  className="h-8 px-3 rounded-lg bg-white/[0.08] border border-[var(--border-subtle)] text-xs font-medium text-[var(--text-primary)] hover:bg-white/[0.12] cursor-pointer transition-colors"
                >
                  Close
                </button>
              </div>
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
};

import React, { useState, useEffect } from "react";
import {
  Settings,
  CalendarClock,
  Gauge,
  ChevronDown,
  ArrowDown,
  ArrowUp,
  Cpu,
  Wifi,
  ArrowLeft,
} from "lucide-react";
import { SpeedLimiterModal } from "./SpeedLimiterModal";
import { formatBytes } from "../utils/format";

export interface StatusFooterProps {
  activeView: "downloads" | "settings";
  onToggleView: () => void;
  onOpenScheduler: () => void;
  speedLimit: string;
  onSpeedLimitChange: (limit: string) => void;
  selectedDownload?: {
    name: string;
    progress: number;
    state: string;
    connections?: number;
  } | null;
  totalSpeed: string;
  totalUploadSpeed: string;
  activeThreads: number;
  totalSessionBytes: number;
}

/**
 * Slim native bottom status bar shared across Downloads and Settings views.
 */
export const StatusFooter: React.FC<StatusFooterProps> = ({
  activeView,
  onToggleView,
  onOpenScheduler,
  speedLimit,
  onSpeedLimitChange,
  selectedDownload,
  totalSpeed,
  totalUploadSpeed,
  activeThreads,
  totalSessionBytes,
}) => {
  const [isLimiterOpen, setIsLimiterOpen] = useState(false);
  const isThrottled = speedLimit !== "Unlimited";
  const isSettings = activeView === "settings";

  useEffect(() => {
    if (!isLimiterOpen) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") setIsLimiterOpen(false);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [isLimiterOpen]);

  return (
    <footer className="h-8 px-2 sm:px-3 border-t border-[var(--border-dim)] bg-[var(--bg-header)] flex items-center justify-between text-[11px] shrink-0 select-none gap-2 w-full z-20">
      {/* Left: Ghost status segments + Selected Task Quick Readout */}
      <div className="flex items-center h-full min-w-0">
        <div className="flex items-center h-full divide-x divide-[var(--border-dim)] shrink-0">
          <button
            onClick={onToggleView}
            className="btn-ghost h-full px-2.5 text-[11px] font-medium inline-flex items-center gap-1.5 whitespace-nowrap shrink-0 cursor-pointer"
            title={isSettings ? "Back to Downloads" : "Settings & Preferences (Ctrl+,)"}
            aria-label={isSettings ? "Back to Downloads" : "Open Settings"}
          >
            {isSettings ? (
              <>
                <ArrowLeft className="w-3.5 h-3.5 text-[var(--text-muted)] shrink-0" />
                <span className="hidden xs:inline">Downloads</span>
              </>
            ) : (
              <>
                <Settings className="w-3.5 h-3.5 text-[var(--text-muted)] shrink-0" />
                <span className="hidden xs:inline">Settings</span>
              </>
            )}
          </button>

          <button
            onClick={onOpenScheduler}
            className="btn-ghost h-full px-2.5 text-[11px] font-medium inline-flex items-center gap-1.5 whitespace-nowrap shrink-0 cursor-pointer"
            title="Download Queues & Scheduler"
            aria-label="Open Scheduler"
          >
            <CalendarClock className="w-3.5 h-3.5 text-[var(--text-muted)] shrink-0" />
            <span className="hidden sm:inline">Scheduler</span>
          </button>

          {/* Speed Limiter Dial (Opens upward from status bar) */}
          <div className="relative h-full shrink-0">
            <button
              onClick={() => setIsLimiterOpen((prev) => !prev)}
              className={`btn-ghost h-full px-2.5 text-[11px] font-medium inline-flex items-center gap-1.5 whitespace-nowrap shrink-0 cursor-pointer ${
                isThrottled ? "!text-[var(--accent)]" : ""
              }`}
              title="Global Bandwidth Limiter"
              aria-label="Global Bandwidth Limiter"
            >
              <Gauge
                className={`w-3.5 h-3.5 shrink-0 ${
                  isThrottled ? "text-[var(--accent)]" : "text-[var(--text-muted)]"
                }`}
              />
              <span>Limit: {speedLimit}</span>
              <ChevronDown
                className={`w-3 h-3 text-[var(--text-muted)] shrink-0 transition-transform ${
                  isLimiterOpen ? "rotate-180" : ""
                }`}
              />
            </button>

            <SpeedLimiterModal
              isOpen={isLimiterOpen}
              speedLimit={speedLimit}
              onSelect={onSpeedLimitChange}
              onClose={() => setIsLimiterOpen(false)}
            />
          </div>
        </div>

        <div className="h-4 w-px bg-[var(--border-dim)] mx-2 shrink-0" />

        {/* Selected Task Quick Readout */}
        <div className="flex items-center gap-2 text-[var(--text-secondary)] min-w-0 truncate">
          {selectedDownload ? (
            <div className="flex items-center gap-1.5 sm:gap-2 min-w-0 truncate">
              <span className="font-semibold text-[var(--text-primary)] truncate max-w-[100px] sm:max-w-xs">
                {selectedDownload.name}
              </span>
              <span className="text-[var(--text-muted)]">•</span>
              <span className="font-mono tabular-nums text-[var(--accent)] whitespace-nowrap">
                {selectedDownload.progress}%
              </span>
              <span className="text-[var(--text-muted)] hidden xs:inline">•</span>
              <span className="hidden xs:inline whitespace-nowrap">
                {selectedDownload.state === "downloading"
                  ? selectedDownload.connections || 0
                  : 0}{" "}
                Active Threads
              </span>
            </div>
          ) : (
            <span className="text-[var(--text-muted)] truncate">No download selected</span>
          )}
        </div>
      </div>

      {/* Right: Aggregate Network Telemetry */}
      <div className="flex items-center gap-2 sm:gap-3.5 font-mono tabular-nums text-[11px] shrink-0">
        <div className="flex items-center gap-1 text-emerald-400 font-semibold whitespace-nowrap">
          <ArrowDown className="w-3.5 h-3.5 stroke-[2.5]" />
          <span>{totalSpeed}</span>
        </div>
        <div
          className={`hidden sm:flex items-center gap-1 whitespace-nowrap ${
            totalUploadSpeed !== "0 B/s"
              ? "text-[#818CF8] font-semibold"
              : "text-[var(--text-muted)]"
          }`}
        >
          <ArrowUp className="w-3.5 h-3.5" />
          <span>{totalUploadSpeed}</span>
        </div>
        <div className="hidden md:block text-[var(--border-subtle)]">|</div>
        <div className="hidden md:flex text-[var(--text-secondary)] items-center gap-1.5 whitespace-nowrap">
          <Cpu className="w-3.5 h-3.5 text-[var(--text-muted)]" />
          <span>{activeThreads} Active Threads</span>
        </div>
        <div className="hidden lg:block text-[var(--border-subtle)]">|</div>
        <div className="hidden lg:flex text-[var(--text-secondary)] items-center gap-1.5 whitespace-nowrap">
          <Wifi className="w-3.5 h-3.5 text-[var(--text-muted)]" />
          <span>Session: {formatBytes(totalSessionBytes)}</span>
        </div>
      </div>
    </footer>
  );
};

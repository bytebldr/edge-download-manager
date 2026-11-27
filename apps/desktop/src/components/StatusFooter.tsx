import React, { useState, useEffect } from "react";
import {
  CalendarClock,
  Gauge,
  ChevronDown,
  ArrowDown,
  ArrowUp,
  Cpu,
  Infinity as InfinityIcon,
} from "lucide-react";
import { SpeedLimiterModal } from "./SpeedLimiterModal";
import { Tooltip } from "./Tooltip";

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
  onOpenScheduler,
  speedLimit,
  onSpeedLimitChange,
  totalSpeed,
  totalUploadSpeed,
  activeThreads,
}) => {
  const [isLimiterOpen, setIsLimiterOpen] = useState(false);
  const isThrottled = speedLimit !== "Unlimited";

  useEffect(() => {
    if (!isLimiterOpen) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") setIsLimiterOpen(false);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [isLimiterOpen]);

  return (
    <footer className="h-11 px-3 sm:px-4 footer-3d flex items-center justify-between text-xs shrink-0 select-none gap-2 w-full z-20">
      {/* Left: Ghost status segments */}
      <div className="flex items-center h-full min-w-0">
        <div className="flex items-center h-full gap-1 shrink-0">
          <Tooltip content="Download Queues & Scheduler" side="top" align="start" className="h-full">
            <button
              onClick={onOpenScheduler}
              className="group h-full px-2.5 text-xs font-medium inline-flex items-center gap-2 whitespace-nowrap shrink-0 cursor-pointer bg-transparent text-[var(--text-secondary)] hover:text-[var(--text-primary)] active:scale-[0.97] transition-[color,transform,filter] duration-150 focus:outline-none focus-visible:text-[var(--accent)]"
              aria-label="Open Scheduler"
            >
              <CalendarClock className="w-4 h-4 stroke-[1.75] text-[var(--text-muted)] group-hover:text-[var(--accent)] group-hover:drop-shadow-[0_0_6px_var(--accent)] transition-[color,filter] shrink-0" />
              <span className="group-hover:drop-shadow-[0_0_8px_rgba(255,255,255,0.22)] transition-[filter]">Scheduler</span>
            </button>
          </Tooltip>

          <div className="h-4 w-px bg-[var(--border-dim)] mx-1 shrink-0" />

          {/* Speed Limiter Dial (Opens upward from status bar) */}
          <div className="relative h-full shrink-0">
            <Tooltip content="Global Bandwidth Limiter" side="top" className="h-full">
              <button
                onClick={() => setIsLimiterOpen((prev) => !prev)}
                className={`group h-full px-2.5 text-xs font-medium inline-flex items-center gap-2 whitespace-nowrap shrink-0 cursor-pointer bg-transparent active:scale-[0.97] transition-[color,transform,filter] duration-150 focus:outline-none ${
                  isThrottled
                    ? "text-[var(--accent)]"
                    : "text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
                }`}
                aria-label="Global Bandwidth Limiter"
              >
                {isThrottled ? (
                  <Gauge className="w-4 h-4 stroke-[1.75] text-[var(--accent)] group-hover:drop-shadow-[0_0_6px_var(--accent)] transition-[filter] shrink-0" />
                ) : (
                  <InfinityIcon className="w-4 h-4 stroke-[1.75] text-[var(--text-muted)] group-hover:text-[var(--accent)] group-hover:drop-shadow-[0_0_6px_var(--accent)] transition-[color,filter] shrink-0" />
                )}
                <span className="group-hover:drop-shadow-[0_0_8px_rgba(255,255,255,0.22)] transition-[filter]">{speedLimit}</span>
                <ChevronDown
                  className={`w-3.5 h-3.5 text-[var(--text-muted)] group-hover:text-[var(--text-primary)] shrink-0 transition-transform ${
                    isLimiterOpen ? "rotate-180" : ""
                  }`}
                />
              </button>
            </Tooltip>

            <SpeedLimiterModal
              isOpen={isLimiterOpen}
              speedLimit={speedLimit}
              onSelect={onSpeedLimitChange}
              onClose={() => setIsLimiterOpen(false)}
            />
          </div>
        </div>
      </div>

      {/* Right: Aggregate Network Telemetry */}
      <div className="flex items-center gap-3 sm:gap-4 tabular-nums text-xs shrink-0">
        <div className="flex items-center gap-1.5 text-[var(--accent)] font-semibold whitespace-nowrap">
          <ArrowDown className="w-3.5 h-3.5 stroke-[2.5]" />
          <span>{totalSpeed}</span>
        </div>
        <div
          className={`hidden sm:flex items-center gap-1.5 whitespace-nowrap ${
            totalUploadSpeed !== "0 B/s"
              ? "text-[#818CF8] font-semibold"
              : "text-[var(--text-secondary)]"
          }`}
        >
          <ArrowUp className="w-3.5 h-3.5 text-[var(--text-muted)]" />
          <span>{totalUploadSpeed}</span>
        </div>
        <div className="hidden md:block text-[var(--border-subtle)]">|</div>
        <div className="hidden md:flex text-[var(--text-secondary)] items-center gap-1.5 whitespace-nowrap">
          <Cpu className="w-4 h-4 stroke-[1.75] text-[var(--text-muted)]" />
          <span>Active Threads: {activeThreads}</span>
        </div>
      </div>
    </footer>
  );
};

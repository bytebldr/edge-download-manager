import React from "react";
import {
  Play,
  Pause,
  Square,
  Trash2,
  Plus,
  Search,
  X,
  Minus,
  ArrowLeft,
  CalendarPlus,
} from "lucide-react";
import { APP_NAME } from "../config/constants";
import { Tooltip } from "./Tooltip";
import {
  tauriStartDragging,
  tauriWindowMinimize,
  tauriWindowToggleMaximize,
  tauriWindowClose,
} from "../services/tauriBridge";

export interface AppToolbarProps {
  activeView?: "downloads" | "settings";
  onBackToDownloads?: () => void;
  selectedCount: number;
  onResumeSelected: () => void;
  onPauseSelected: () => void;
  onStopSelected: () => void;
  onDeleteSelected: () => void;
  onAddUrl: () => void;
  onAddToScheduler?: () => void;
  searchQuery: string;
  onSearchChange: (value: string) => void;
  viewMode: "table" | "grid";
  onViewModeChange: (mode: "table" | "grid") => void;
}

/**
 * Top window header & command deck with brand identity, task actions, search, view toggle, and window controls.
 */
export const AppToolbar: React.FC<AppToolbarProps> = ({
  activeView = "downloads",
  onBackToDownloads,
  selectedCount,
  onResumeSelected,
  onPauseSelected,
  onStopSelected,
  onDeleteSelected,
  onAddUrl,
  onAddToScheduler,
  searchQuery,
  onSearchChange,
}) => {
  const hasSelection = selectedCount > 0;
  const isSettings = activeView === "settings";
  const actionBtnClass = (hoverColor: string) =>
    `h-8 w-8 rounded-lg flex items-center justify-center shrink-0 transition-colors border ${
      hasSelection
        ? `bg-white/[0.04] border-[var(--border-subtle)] text-[var(--text-secondary)] ${hoverColor} hover:bg-white/[0.08] cursor-pointer`
        : "bg-transparent border-transparent opacity-40 cursor-not-allowed text-[var(--text-muted)]"
    }`;

  return (
    <header
      data-tauri-drag-region
      onMouseDown={(e) => {
        if (e.button === 0 && !(e.target as HTMLElement).closest("button, input, select, a, textarea")) {
          tauriStartDragging();
        }
      }}
      onDoubleClick={(e) => {
        if (!(e.target as HTMLElement).closest("button, input, select, a, textarea")) {
          tauriWindowToggleMaximize();
        }
      }}
      className="h-14 px-3 sm:px-4 header-3d flex items-center justify-between gap-2 shrink-0 z-20 select-none relative"
    >
      {/* Left: Brand Identity with Official 3D Logo & Context Actions */}
      <div className="flex items-center gap-2 sm:gap-2.5 shrink-0 z-10">
        {isSettings ? (
          <>
            <Tooltip content="Back to Downloads" side="bottom" align="start">
              <button
                onClick={onBackToDownloads}
                className="h-8 px-2.5 rounded-lg flex items-center gap-1.5 text-xs font-medium text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-card-hover)] transition-colors cursor-pointer shrink-0"
                aria-label="Back to Downloads"
              >
                <ArrowLeft className="w-3.5 h-3.5" />
                <span>Downloads</span>
              </button>
            </Tooltip>
            <div className="h-4 w-px bg-[var(--border-subtle)] shrink-0" />
            <div className="flex items-center gap-2 shrink-0">
              <img
                src="/logo.png"
                alt={`${APP_NAME} Logo`}
                className="w-6 h-6 object-contain shrink-0 pointer-events-none"
              />
              <span className="font-semibold tracking-tight text-xs sm:text-sm text-[var(--text-primary)]">
                Preferences
              </span>
            </div>
          </>
        ) : (
          <>
            <div className="flex items-center gap-2.5 shrink-0">
              <img
                src="/logo.png"
                alt={`${APP_NAME} Logo`}
                className="w-7 h-7 object-contain shrink-0 pointer-events-none"
              />
              <span className="font-semibold tracking-tight text-sm text-[var(--text-primary)]">
                {APP_NAME}
              </span>
            </div>
          </>
        )}
      </div>

      {/* Center: Search Bar pinned at exact geometric center; Controls on its left (when selected), + Add URL on its right */}
      {!isSettings && (
        <div
          className="absolute left-1/2 -translate-x-1/2 flex items-center z-10"
          data-tauri-drag-region
        >
          {/* Left Wing of Search: Task Controls (Appear/disappear on selection without shifting Search) */}
          {hasSelection && (
            <div className="absolute right-full mr-2.5 flex items-center gap-1 pr-2.5 border-r border-[var(--border-subtle)] shrink-0">
              <Tooltip content="Resume Selected" side="bottom">
                <button
                  onClick={onResumeSelected}
                  className={actionBtnClass("hover:text-emerald-400")}
                  aria-label="Resume selected tasks"
                >
                  <Play className="w-3.5 h-3.5 text-emerald-400 fill-emerald-400/20" />
                </button>
              </Tooltip>

              <Tooltip content="Pause Selected" side="bottom">
                <button
                  onClick={onPauseSelected}
                  className={actionBtnClass("hover:text-amber-400")}
                  aria-label="Pause selected tasks"
                >
                  <Pause className="w-3.5 h-3.5 text-amber-400 fill-amber-400/20" />
                </button>
              </Tooltip>

              <Tooltip content="Stop Selected" side="bottom">
                <button
                  onClick={onStopSelected}
                  className={actionBtnClass("hover:text-rose-400")}
                  aria-label="Stop selected downloads"
                >
                  <Square className="w-3 h-3 text-rose-400 fill-rose-400/20" />
                </button>
              </Tooltip>

              <Tooltip content="Delete Selected" side="bottom">
                <button
                  onClick={onDeleteSelected}
                  className={actionBtnClass("hover:text-rose-400")}
                  aria-label="Delete selected tasks"
                >
                  <Trash2 className="w-3.5 h-3.5" />
                </button>
              </Tooltip>
            </div>
          )}

          {/* Dead-Center: Search Box */}
          <div className="relative shrink-0">
            <Search className="w-4 h-4 stroke-[1.75] absolute left-3 top-1/2 -translate-y-1/2 text-[var(--text-muted)] pointer-events-none" />
            <input
              type="text"
              placeholder="Search"
              aria-label="Search downloads"
              value={searchQuery}
              onChange={(e) => onSearchChange(e.target.value)}
              className="w-44 sm:w-60 md:w-72 h-8 pl-9 pr-7 rounded-lg text-xs bg-white/[0.03] border border-[var(--border-subtle)] text-[var(--text-primary)] placeholder-[var(--text-muted)] focus:outline-none focus-visible:ring-2 focus-visible:ring-[var(--accent)]/40 focus:border-[var(--accent)]/60 transition-colors truncate"
            />
            {searchQuery && (
              <Tooltip content="Clear Search" side="bottom" className="!absolute right-2 top-1/2 -translate-y-1/2">
                <button
                  onClick={() => onSearchChange("")}
                  className="text-[var(--text-muted)] hover:text-[var(--text-primary)] cursor-pointer transition-colors flex items-center justify-center"
                  aria-label="Clear search"
                >
                  <X className="w-3.5 h-3.5 stroke-[1.75]" />
                </button>
              </Tooltip>
            )}
          </div>

          {/* Right Wing of Search: Icon-only Add URL + Add to Scheduler buttons */}
          <div className="absolute left-full ml-2.5 flex items-center gap-1.5 shrink-0">
            <Tooltip content="Add Download URL (Ctrl+N)" side="bottom">
              <button
                onClick={onAddUrl}
                className="h-8 w-8 rounded-lg bg-white/[0.04] border border-[var(--border-subtle)] text-[var(--accent)] hover:bg-white/[0.08] hover:border-[var(--accent)]/40 active:scale-[0.97] flex items-center justify-center shrink-0 cursor-pointer transition-colors"
                aria-label="Add Download URL"
              >
                <Plus className="w-4 h-4 stroke-[2]" />
              </button>
            </Tooltip>
            {onAddToScheduler && (
              <Tooltip content="Add to Scheduler" side="bottom">
                <button
                  onClick={onAddToScheduler}
                  className="h-8 w-8 rounded-lg bg-white/[0.04] border border-[var(--border-subtle)] text-[var(--text-secondary)] hover:text-[var(--accent)] hover:bg-white/[0.08] hover:border-[var(--accent)]/40 active:scale-[0.97] flex items-center justify-center shrink-0 cursor-pointer transition-colors"
                  aria-label="Add to Scheduler"
                >
                  <CalendarPlus className="w-4 h-4 stroke-[1.75]" />
                </button>
              </Tooltip>
            )}
          </div>
        </div>
      )}

      {/* Right: Unified Integrated Window Controls */}
      <div className="flex items-center gap-1.5 shrink-0 z-10 ml-auto" data-tauri-drag-region="false">
        <Tooltip content="Minimize" side="bottom">
          <button
            onClick={(e) => {
              e.stopPropagation();
              tauriWindowMinimize();
            }}
            aria-label="Minimize window"
            className="h-8 w-8 rounded-lg flex items-center justify-center text-[var(--text-muted)] hover:text-[var(--text-primary)] hover:bg-white/10 transition-colors cursor-pointer"
          >
            <Minus className="w-3.5 h-3.5" />
          </button>
        </Tooltip>
        <Tooltip content="Maximize / Restore" side="bottom">
          <button
            onClick={(e) => {
              e.stopPropagation();
              tauriWindowToggleMaximize();
            }}
            aria-label="Maximize or restore window"
            className="h-8 w-8 rounded-lg flex items-center justify-center text-[var(--text-muted)] hover:text-[var(--text-primary)] hover:bg-white/10 transition-colors cursor-pointer"
          >
            <Square className="w-3 h-3" />
          </button>
        </Tooltip>
        <Tooltip content="Close" side="bottom" align="end">
          <button
            onClick={(e) => {
              e.stopPropagation();
              tauriWindowClose();
            }}
            aria-label="Close window"
            className="h-8 w-8 rounded-lg border border-[var(--accent)]/45 flex items-center justify-center text-[var(--text-secondary)] hover:text-white hover:bg-rose-600 hover:border-rose-600 transition-colors cursor-pointer"
          >
            <X className="w-3.5 h-3.5" />
          </button>
        </Tooltip>
      </div>
    </header>
  );
};

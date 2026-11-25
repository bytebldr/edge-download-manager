import React from "react";
import {
  Play,
  Pause,
  Square,
  Trash2,
  Plus,
  Search,
  X,
  LayoutList,
  LayoutGrid,
  Minus,
} from "lucide-react";
import { APP_NAME, APP_SUBTITLE } from "../config/constants";
import {
  tauriStartDragging,
  tauriWindowMinimize,
  tauriWindowToggleMaximize,
  tauriWindowClose,
} from "../services/tauriBridge";

export interface AppToolbarProps {
  selectedCount: number;
  onResumeSelected: () => void;
  onPauseSelected: () => void;
  onStopSelected: () => void;
  onDeleteSelected: () => void;
  onAddUrl: () => void;
  searchQuery: string;
  onSearchChange: (value: string) => void;
  viewMode: "table" | "grid";
  onViewModeChange: (mode: "table" | "grid") => void;
}

/**
 * Top window header & command deck with brand identity, task actions, search, view toggle, and window controls.
 */
export const AppToolbar: React.FC<AppToolbarProps> = ({
  selectedCount,
  onResumeSelected,
  onPauseSelected,
  onStopSelected,
  onDeleteSelected,
  onAddUrl,
  searchQuery,
  onSearchChange,
  viewMode,
  onViewModeChange,
}) => {
  const hasSelection = selectedCount > 0;
  const actionBtnClass = (hoverColor: string) =>
    `btn-3d h-8 w-8 rounded-lg flex items-center justify-center shrink-0 transition-all ${
      hasSelection
        ? `text-[var(--text-secondary)] ${hoverColor} cursor-pointer`
        : "opacity-30 cursor-not-allowed text-[var(--text-muted)] pointer-events-none"
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
        <div className="flex items-center gap-2 sm:gap-2.5 shrink-0">
          <img
            src="/logo.png"
            alt={`${APP_NAME} Logo`}
            className="w-7 h-7 sm:w-8 sm:h-8 object-contain drop-shadow-md transition-transform hover:scale-105 duration-150 shrink-0 pointer-events-none"
          />
          <div className="shrink-0 flex flex-col justify-center">
            <span className="font-bold tracking-tight text-xs sm:text-sm">{APP_NAME}</span>
            <div className="text-[10px] text-[var(--text-muted)] whitespace-nowrap font-medium leading-tight mt-0.5 hidden xs:block">
              {APP_SUBTITLE}
            </div>
          </div>
        </div>

        {/* Contextual Toolbar (Always visible; grayed out when no task is selected) */}
        <div className="flex items-center gap-1 shrink-0 pl-2 border-l border-[var(--border-subtle)]">
          <button
            disabled={!hasSelection}
            onClick={onResumeSelected}
            className={actionBtnClass("hover:text-emerald-400")}
            title={hasSelection ? "Resume Selected Tasks" : "Resume (Select task first)"}
          >
            <Play className="w-3.5 h-3.5 text-emerald-400 fill-emerald-400/20" />
          </button>

          <button
            disabled={!hasSelection}
            onClick={onPauseSelected}
            className={actionBtnClass("hover:text-amber-400")}
            title={hasSelection ? "Pause Selected Tasks" : "Pause (Select task first)"}
          >
            <Pause className="w-3.5 h-3.5 text-amber-400 fill-amber-400/20" />
          </button>

          <button
            disabled={!hasSelection}
            onClick={onStopSelected}
            className={actionBtnClass("hover:text-rose-400")}
            title={hasSelection ? "Stop Selected Downloads" : "Stop (Select task first)"}
          >
            <Square className="w-3 h-3 text-rose-400 fill-rose-400/20" />
          </button>

          <button
            disabled={!hasSelection}
            onClick={onDeleteSelected}
            className={actionBtnClass("hover:text-rose-400")}
            title={hasSelection ? "Delete Selected Tasks" : "Delete (Select task first)"}
          >
            <Trash2 className="w-3.5 h-3.5" />
          </button>
        </div>
      </div>

      {/* Center: Command Deck Pinned to Exact Geometric Center */}
      <div
        className="absolute left-1/2 -translate-x-1/2 flex items-center gap-1.5 sm:gap-2 shrink-0 z-10"
        data-tauri-drag-region
      >
        {/* Primary Action: Add URL */}
        <button
          onClick={onAddUrl}
          className="btn-primary-3d h-8 px-2.5 rounded-lg flex items-center gap-1.5 shrink-0 cursor-pointer text-xs font-semibold"
          title="Add Download URL (Ctrl+N)"
        >
          <Plus className="w-4 h-4 stroke-[2.5]" />
          <span className="hidden sm:inline">Add URL</span>
        </button>

        {/* Search Box */}
        <div className="relative shrink-0">
          <Search className="w-3.5 h-3.5 absolute left-2.5 top-1/2 -translate-y-1/2 text-[var(--text-muted)] pointer-events-none icon-3d" />
          <input
            type="text"
            placeholder="Filter tasks..."
            value={searchQuery}
            onChange={(e) => onSearchChange(e.target.value)}
            className="w-28 sm:w-44 md:w-56 focus:w-36 sm:focus:w-52 md:focus:w-64 h-8 pl-8 pr-7 rounded-lg text-xs input-3d text-[var(--text-primary)] placeholder-[var(--text-muted)] focus:outline-none transition-all truncate"
          />
          {searchQuery && (
            <button
              onClick={() => onSearchChange("")}
              className="absolute right-2 top-1/2 -translate-y-1/2 text-[var(--text-muted)] hover:text-[var(--text-primary)] cursor-pointer"
            >
              <X className="w-3 h-3" />
            </button>
          )}
        </div>

        {/* View Mode Toggle */}
        <div className="h-8 flex items-center p-0.5 rounded-lg input-3d shrink-0">
          <button
            onClick={() => onViewModeChange("table")}
            className={`h-full px-2 rounded-md cursor-pointer transition-all inline-flex items-center ${
              viewMode === "table"
                ? "btn-3d text-[var(--accent)] shadow-sm font-semibold"
                : "text-[var(--text-muted)] hover:text-[var(--text-primary)]"
            }`}
            title="Table View"
          >
            <LayoutList className="w-3.5 h-3.5 icon-3d" />
          </button>
          <button
            onClick={() => onViewModeChange("grid")}
            className={`h-full px-2 rounded-md cursor-pointer transition-all inline-flex items-center ${
              viewMode === "grid"
                ? "btn-3d text-[var(--accent)] shadow-sm font-semibold"
                : "text-[var(--text-muted)] hover:text-[var(--text-primary)]"
            }`}
            title="Grid View"
          >
            <LayoutGrid className="w-3.5 h-3.5 icon-3d" />
          </button>
        </div>
      </div>

      {/* Right: Unified Integrated Window Controls & Quick Actions */}
      <div className="flex items-center gap-1 shrink-0 z-10 ml-auto" data-tauri-drag-region="false">
        <button
          onClick={(e) => {
            e.stopPropagation();
            tauriWindowMinimize();
          }}
          className="h-8 w-8 rounded-lg flex items-center justify-center text-[var(--text-muted)] hover:text-[var(--text-primary)] hover:bg-white/10 transition-colors cursor-pointer"
          title="Minimize"
        >
          <Minus className="w-3.5 h-3.5" />
        </button>
        <button
          onClick={(e) => {
            e.stopPropagation();
            tauriWindowToggleMaximize();
          }}
          className="h-8 w-8 rounded-lg flex items-center justify-center text-[var(--text-muted)] hover:text-[var(--text-primary)] hover:bg-white/10 transition-colors cursor-pointer"
          title="Maximize / Restore"
        >
          <Square className="w-3 h-3" />
        </button>
        <button
          onClick={(e) => {
            e.stopPropagation();
            tauriWindowClose();
          }}
          className="h-8 w-8 rounded-lg flex items-center justify-center text-[var(--text-muted)] hover:text-white hover:bg-rose-600 transition-colors cursor-pointer"
          title="Close"
        >
          <X className="w-3.5 h-3.5" />
        </button>
      </div>
    </header>
  );
};

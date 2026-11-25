import React, { useState } from "react";
import {
  CheckCircle2,
  Pause,
  Play,
  FolderOpen,
  Copy,
  Check,
  Layers,
  FileCode,
  Trash2,
} from "lucide-react";
import type { DownloadItem } from "../App";

export interface DownloadContextMenuProps {
  visible: boolean;
  x: number;
  y: number;
  item: DownloadItem | null;
  onClose: () => void;
  onOpenFile: (id: string) => void;
  onPause: (id: string) => void;
  onResume: (id: string) => void;
  onOpenFolder: (id: string) => void;
  onInspect: (item: DownloadItem) => void;
  onEditRequest: (item: DownloadItem) => void;
  onDelete: (id: string, name: string) => void;
}

/**
 * Floating right-click context menu for download rows and cards.
 */
export const DownloadContextMenu: React.FC<DownloadContextMenuProps> = ({
  visible,
  x,
  y,
  item,
  onClose,
  onOpenFile,
  onPause,
  onResume,
  onOpenFolder,
  onInspect,
  onEditRequest,
  onDelete,
}) => {
  const [copiedId, setCopiedId] = useState<string | null>(null);

  if (!visible || !item) return null;

  const isCompleted = item.state === "completed" || item.progress >= 100;

  return (
    <div
      style={{ top: `${y}px`, left: `${x}px` }}
      className="fixed z-50 w-56 rounded-xl panel-3d shadow-2xl p-1.5 text-xs border border-[var(--border-subtle)] bg-[var(--bg-card)]/95 backdrop-blur-md select-none"
      onClick={(e) => e.stopPropagation()}
    >
      <div className="px-2.5 py-1.5 border-b border-[var(--border-subtle)]/70 text-[10px] font-mono font-medium text-[var(--text-muted)] truncate">
        {item.name}
      </div>
      <div className="py-1 space-y-0.5">
        {isCompleted ? (
          <button
            onClick={() => {
              onOpenFile(item.id);
              onClose();
            }}
            className="w-full px-2.5 py-1.5 rounded-lg flex items-center gap-2 hover:bg-white/5 text-[var(--text-primary)] cursor-pointer transition-colors"
          >
            <CheckCircle2 className="w-3.5 h-3.5 text-emerald-400" />
            <span>Open File</span>
          </button>
        ) : item.state === "downloading" ? (
          <button
            onClick={() => {
              onPause(item.id);
              onClose();
            }}
            className="w-full px-2.5 py-1.5 rounded-lg flex items-center gap-2 hover:bg-white/5 text-amber-400 cursor-pointer transition-colors"
          >
            <Pause className="w-3.5 h-3.5" />
            <span>Pause Download</span>
          </button>
        ) : (
          <button
            onClick={() => {
              onResume(item.id);
              onClose();
            }}
            className="w-full px-2.5 py-1.5 rounded-lg flex items-center gap-2 hover:bg-white/5 text-emerald-400 cursor-pointer transition-colors"
          >
            <Play className="w-3.5 h-3.5" />
            <span>Resume Download</span>
          </button>
        )}

        <button
          onClick={() => {
            onOpenFolder(item.id);
            onClose();
          }}
          className="w-full px-2.5 py-1.5 rounded-lg flex items-center gap-2 hover:bg-white/5 text-[var(--text-secondary)] hover:text-[var(--text-primary)] cursor-pointer transition-colors"
        >
          <FolderOpen className="w-3.5 h-3.5 text-[var(--accent)]" />
          <span>Open Folder</span>
        </button>

        <button
          onClick={() => {
            navigator.clipboard.writeText(item.url);
            setCopiedId(item.id);
            setTimeout(() => {
              setCopiedId(null);
              onClose();
            }, 800);
          }}
          className="w-full px-2.5 py-1.5 rounded-lg flex items-center gap-2 hover:bg-white/5 text-[var(--text-secondary)] hover:text-[var(--text-primary)] cursor-pointer transition-colors"
        >
          {copiedId === item.id ? (
            <>
              <Check className="w-3.5 h-3.5 text-emerald-400" />
              <span className="text-emerald-400">Copied URL!</span>
            </>
          ) : (
            <>
              <Copy className="w-3.5 h-3.5" />
              <span>Copy Source URL</span>
            </>
          )}
        </button>

        <button
          onClick={() => {
            onInspect(item);
            onClose();
          }}
          className="w-full px-2.5 py-1.5 rounded-lg flex items-center gap-2 hover:bg-white/5 text-[var(--text-secondary)] hover:text-[var(--text-primary)] cursor-pointer transition-colors"
        >
          <Layers className="w-3.5 h-3.5 text-[var(--accent)]" />
          <span>Inspect Streams</span>
          <span className="ml-auto text-[10px] text-[var(--text-muted)] font-mono">Enter</span>
        </button>

        <button
          onClick={() => {
            onEditRequest(item);
            onClose();
          }}
          className="w-full px-2.5 py-1.5 rounded-lg flex items-center gap-2 hover:bg-white/5 text-[var(--text-secondary)] hover:text-[var(--text-primary)] cursor-pointer transition-colors"
        >
          <FileCode className="w-3.5 h-3.5 text-[var(--accent)]" />
          <span>Edit Request / Headers</span>
        </button>
      </div>

      <div className="pt-1 mt-1 border-t border-[var(--border-subtle)]/70">
        <button
          onClick={() => {
            onClose();
            onDelete(item.id, item.name);
          }}
          className="w-full px-2.5 py-1.5 rounded-lg flex items-center gap-2 text-rose-400 hover:bg-rose-500/10 cursor-pointer transition-colors"
        >
          <Trash2 className="w-3.5 h-3.5" />
          <span>Delete from List</span>
          <span className="ml-auto text-[10px] text-rose-400/70 font-mono">Del</span>
        </button>
      </div>
    </div>
  );
};

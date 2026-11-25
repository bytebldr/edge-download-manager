import React, { useState } from "react";
import { X, Plus, Trash2, ShieldAlert, Check } from "lucide-react";
import { motion, AnimatePresence } from "motion/react";
import { backdropVariants, modalVariants } from "../../utils/motion";
import { isTauri } from "../../services/tauriBridge";

export interface RequestOverride {
  url: string | null;
  headers: Array<[string, string]>;
  proxyUrl: string | null;
}

export interface EditRequestModalProps {
  isOpen: boolean;
  taskId: string;
  initialUrl?: string;
  initialHeaders?: Array<[string, string]>;
  initialProxyUrl?: string | null;
  onClose: () => void;
  onSaved?: (override: RequestOverride) => void | Promise<void>;
}

const PRESET_HEADERS = ["Cookie", "Referer", "Authorization"] as const;

export const EditRequestModal: React.FC<EditRequestModalProps> = ({
  isOpen,
  taskId,
  initialUrl = "",
  initialHeaders = [],
  initialProxyUrl = "",
  onClose,
  onSaved,
}) => {
  const [url, setUrl] = useState(initialUrl);
  const [proxyUrl, setProxyUrl] = useState(initialProxyUrl ?? "");
  const [headers, setHeaders] = useState<Array<[string, string]>>(initialHeaders);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!isOpen) return null;

  const addHeaderRow = (name = "") => {
    setHeaders((prev) => [...prev, [name, ""]]);
  };

  const updateHeader = (index: number, col: 0 | 1, value: string) => {
    setHeaders((prev) =>
      prev.map((row, i) => (i === index ? (col === 0 ? [value, row[1]] : [row[0], value]) : row))
    );
  };

  const removeHeader = (index: number) => {
    setHeaders((prev) => prev.filter((_, i) => i !== index));
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);

    const cleanUrl = url.trim();
    if (cleanUrl) {
      try {
        const parsed = new URL(cleanUrl);
        if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
          setError("Override URL must use http:// or https://");
          return;
        }
      } catch {
        setError("Please enter a valid HTTP or HTTPS URL.");
        return;
      }
    }

    const cleanHeaders: Array<[string, string]> = [];
    for (const [k, v] of headers) {
      const name = k.trim();
      if (!name && !v.trim()) continue;
      if (!name) {
        setError("Header name cannot be empty.");
        return;
      }
      if (/[\r\n\0]/.test(name) || /[\r\n\0]/.test(v)) {
        setError(`Header "${name}" contains illegal line break or NUL characters.`);
        return;
      }
      cleanHeaders.push([name, v]);
    }

    const payload: RequestOverride = {
      url: cleanUrl || null,
      headers: cleanHeaders,
      proxyUrl: proxyUrl.trim() || null,
    };

    setIsSaving(true);
    try {
      if (isTauri()) {
        const { invoke } = await import("@tauri-apps/api/core");
        await invoke("update_task_request_override", {
          taskId,
          requestOverride: payload,
        });
      } else {
        localStorage.setItem(`task_request_override_${taskId}`, JSON.stringify(payload));
      }
      await onSaved?.(payload);
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <AnimatePresence>
      <motion.div
        variants={backdropVariants}
        initial="hidden"
        animate="visible"
        exit="exit"
        className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4"
      >
        <motion.div
          variants={modalVariants}
          initial="hidden"
          animate="visible"
          exit="exit"
          className="w-full max-w-lg rounded-2xl bg-[var(--bg-panel)] border border-[var(--border-subtle)] text-[var(--text-primary)] shadow-2xl overflow-hidden"
        >
          <div className="flex items-center justify-between px-5 py-4 border-b border-[var(--border-subtle)]">
            <div>
              <h3 className="text-sm font-semibold">Edit Request Override</h3>
              <p className="text-xs text-[var(--text-muted)]">
                Update source URL, custom headers, or proxy for task {taskId.slice(0, 8)}.
              </p>
            </div>
            <button
              type="button"
              onClick={onClose}
              className="p-1.5 rounded-lg text-[var(--text-muted)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-card)] cursor-pointer"
              aria-label="Close modal"
            >
              <X className="w-4 h-4" />
            </button>
          </div>

          <form onSubmit={handleSubmit} className="flex flex-col gap-4 p-5">
            {error && (
              <div className="flex items-center gap-2 px-3 py-2 rounded-lg text-xs border border-[var(--danger)]/40 bg-[var(--danger)]/10 text-[var(--danger)]">
                <ShieldAlert className="w-4 h-4 shrink-0" />
                <span>{error}</span>
              </div>
            )}

            <label className="flex flex-col gap-1.5 text-xs">
              <span className="font-medium text-[var(--text-secondary)]">Download URL</span>
              <input
                type="url"
                value={url}
                onChange={(e) => setUrl(e.target.value)}
                placeholder="https://cdn.example.com/file.zip"
                className="px-3 py-2 rounded-lg bg-[var(--bg-card)] border border-[var(--border-subtle)] text-[var(--text-primary)]"
              />
            </label>

            <label className="flex flex-col gap-1.5 text-xs">
              <span className="font-medium text-[var(--text-secondary)]">Proxy Override</span>
              <input
                type="text"
                value={proxyUrl}
                onChange={(e) => setProxyUrl(e.target.value)}
                placeholder="socks5://127.0.0.1:1080 (leave empty for default)"
                className="px-3 py-2 rounded-lg bg-[var(--bg-card)] border border-[var(--border-subtle)] text-[var(--text-primary)]"
              />
            </label>

            <div className="flex flex-col gap-2">
              <div className="flex items-center justify-between">
                <span className="text-xs font-medium text-[var(--text-secondary)]">
                  Custom Headers
                </span>
                <div className="flex items-center gap-1">
                  {PRESET_HEADERS.map((preset) => (
                    <button
                      key={preset}
                      type="button"
                      onClick={() => addHeaderRow(preset)}
                      className="px-2 py-0.5 rounded text-[11px] bg-[var(--bg-card)] border border-[var(--border-subtle)] text-[var(--text-muted)] hover:text-[var(--text-primary)] cursor-pointer"
                    >
                      + {preset}
                    </button>
                  ))}
                  <button
                    type="button"
                    onClick={() => addHeaderRow("")}
                    className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-[11px] bg-[var(--accent)]/15 text-[var(--accent)] cursor-pointer"
                  >
                    <Plus className="w-3 h-3" />
                    Custom
                  </button>
                </div>
              </div>

              {headers.length === 0 ? (
                <div className="text-xs text-[var(--text-muted)] py-1">
                  No custom headers configured.
                </div>
              ) : (
                <div className="flex flex-col gap-2 max-h-44 overflow-y-auto">
                  {headers.map(([name, val], idx) => (
                    <div key={idx} className="flex items-center gap-2 text-xs">
                      <input
                        type="text"
                        value={name}
                        onChange={(e) => updateHeader(idx, 0, e.target.value)}
                        placeholder="Header"
                        className="w-1/3 px-2.5 py-1.5 rounded-lg bg-[var(--bg-card)] border border-[var(--border-subtle)] font-mono"
                      />
                      <input
                        type="text"
                        value={val}
                        onChange={(e) => updateHeader(idx, 1, e.target.value)}
                        placeholder="Value"
                        className="flex-1 px-2.5 py-1.5 rounded-lg bg-[var(--bg-card)] border border-[var(--border-subtle)] font-mono"
                      />
                      <button
                        type="button"
                        onClick={() => removeHeader(idx)}
                        className="p-1.5 rounded-lg text-[var(--text-muted)] hover:text-[var(--danger)] cursor-pointer"
                        aria-label="Remove header"
                      >
                        <Trash2 className="w-3.5 h-3.5" />
                      </button>
                    </div>
                  ))}
                </div>
              )}
            </div>

            <div className="flex items-center justify-end gap-2 pt-2 border-t border-[var(--border-subtle)]">
              <button
                type="button"
                onClick={onClose}
                className="px-3 py-1.5 rounded-lg text-xs text-[var(--text-secondary)] hover:bg-[var(--bg-card)] cursor-pointer"
              >
                Cancel
              </button>
              <button
                type="submit"
                disabled={isSaving}
                className="inline-flex items-center gap-1.5 px-3.5 py-1.5 rounded-lg text-xs font-medium bg-[var(--accent)] text-[var(--accent-on)] hover:bg-[var(--accent-hover)] disabled:opacity-50 cursor-pointer"
              >
                <Check className="w-3.5 h-3.5" />
                {isSaving ? "Saving..." : "Apply Override"}
              </button>
            </div>
          </form>
        </motion.div>
      </motion.div>
    </AnimatePresence>
  );
};

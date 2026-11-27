import React, { useMemo, useRef, useState } from "react";
import { Select } from "../../components/Select";
import {
  X,
  Upload,
  FileText,
  Globe,
  CheckSquare,
  Square,
  Download,
  AlertCircle,
} from "lucide-react";
import { motion, AnimatePresence } from "motion/react";
import { backdropVariants, modalVariants } from "../../utils/motion";
import { isTauri } from "../../services/tauriBridge";

export interface ImportedLink {
  url: string;
  filename_hint?: string | null;
  category_hint?: string | null;
}

export interface BatchImportModalProps {
  isOpen: boolean;
  onClose: () => void;
  onQueueSelected: (links: ImportedLink[]) => void | Promise<void>;
  extensionLinks?: ImportedLink[];
}

type ImportFormat = "txt" | "csv" | "html";

const MAX_CLIENT_BYTES = 5 * 1024 * 1024;

function extractExtension(url: string): string {
  const lower = url.trim().toLowerCase();
  if (lower.startsWith("magnet:")) return "magnet";
  try {
    const pathname = new URL(url).pathname;
    const lastSeg = pathname.split("/").pop() || "";
    const dotIdx = lastSeg.lastIndexOf(".");
    if (dotIdx > 0 && dotIdx < lastSeg.length - 1) {
      const ext = lastSeg.slice(dotIdx).toLowerCase();
      if (/^\.[a-z0-9]{1,8}$/.test(ext)) return ext;
    }
  } catch {
    // Fall through to empty
  }
  return "";
}

function inferFormat(text: string, fileName?: string): ImportFormat {
  if (fileName) {
    const ext = fileName.split(".").pop()?.toLowerCase();
    if (ext === "csv") return "csv";
    if (ext === "html" || ext === "htm") return "html";
    if (ext === "txt") return "txt";
  }
  const trimmed = text.trim();
  if (/<(a|video|audio|source|html|body)\b/i.test(trimmed)) return "html";
  if (trimmed.includes(",") && /^[^,\s]+,/.test(trimmed)) return "csv";
  return "txt";
}

function fallbackParseLinks(content: string, format: ImportFormat): ImportedLink[] {
  const seen = new Set<string>();
  const out: ImportedLink[] = [];
  const pushValid = (url: string, filename_hint?: string, category_hint?: string) => {
    const u = url.trim();
    if (!/^(https?:\/\/\S+|magnet:\S+)/i.test(u) || seen.has(u)) return;
    seen.add(u);
    out.push({
      url: u,
      filename_hint: filename_hint?.trim() || null,
      category_hint: category_hint?.trim() || null,
    });
  };

  if (format === "html") {
    const attrRe = /\b(?:href|src)\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))/gi;
    let m: RegExpExecArray | null;
    while ((m = attrRe.exec(content)) !== null) {
      pushValid((m[1] ?? m[2] ?? m[3] ?? "").replace(/&amp;/g, "&"));
    }
    return out;
  }

  for (const rawLine of content.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line || line.startsWith("#")) continue;
    if (format === "csv") {
      const parts = line.split(",").map((p) => p.trim().replace(/^"|"$/g, ""));
      pushValid(parts[0] || "", parts[1], parts[2]);
    } else {
      pushValid(line);
    }
  }
  return out;
}

export const BatchImportModal: React.FC<BatchImportModalProps> = ({
  isOpen,
  onClose,
  onQueueSelected,
  extensionLinks = [],
}) => {
  const fileInputRef = useRef<HTMLInputElement | null>(null);
  const [sourceTab, setSourceTab] = useState<"file" | "extension">(
    extensionLinks.length > 0 ? "extension" : "file"
  );
  const [rawText, setRawText] = useState("");
  const [format, setFormat] = useState<"auto" | ImportFormat>("auto");
  const [parsedLinks, setParsedLinks] = useState<ImportedLink[]>([]);
  const [selectedUrls, setSelectedUrls] = useState<Set<string>>(new Set());
  const [extFilter, setExtFilter] = useState<string>("all");
  const [isDragging, setIsDragging] = useState(false);
  const [isParsing, setIsParsing] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  const activeLinks = sourceTab === "extension" ? extensionLinks : parsedLinks;

  const availableExtensions = useMemo(() => {
    const counts = new Map<string, number>();
    for (const link of activeLinks) {
      const ext = extractExtension(link.url);
      if (ext) counts.set(ext, (counts.get(ext) || 0) + 1);
    }
    return Array.from(counts.entries()).sort((a, b) => b[1] - a[1]);
  }, [activeLinks]);

  const filteredLinks = useMemo(() => {
    if (extFilter === "all") return activeLinks;
    return activeLinks.filter((l) => extractExtension(l.url) === extFilter);
  }, [activeLinks, extFilter]);

  if (!isOpen) return null;

  const parseContent = async (content: string, fileName?: string) => {
    if (!content.trim()) {
      setParsedLinks([]);
      setSelectedUrls(new Set());
      setErrorMsg(null);
      return;
    }
    if (new Blob([content]).size > MAX_CLIENT_BYTES) {
      setErrorMsg("Input exceeds the 5 MiB batch import limit.");
      return;
    }

    const resolvedFormat: ImportFormat =
      format === "auto" ? inferFormat(content, fileName) : format;

    setIsParsing(true);
    setErrorMsg(null);
    try {
      let links: ImportedLink[];
      if (isTauri()) {
        const { invoke } = await import("@tauri-apps/api/core");
        links = await invoke<ImportedLink[]>("parse_batch_import_text", {
          content,
          format: resolvedFormat,
        });
      } else {
        links = fallbackParseLinks(content, resolvedFormat);
      }
      setParsedLinks(links);
      setSelectedUrls(new Set(links.map((l) => l.url)));
      setExtFilter("all");
      if (links.length === 0) {
        setErrorMsg("No valid http://, https://, or magnet: links found.");
      }
    } catch (err: unknown) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setIsParsing(false);
    }
  };

  const handleFile = async (file: File) => {
    if (file.size > MAX_CLIENT_BYTES) {
      setErrorMsg("Selected file exceeds 5 MiB limit.");
      return;
    }
    const text = await file.text();
    setRawText(text);
    setSourceTab("file");
    await parseContent(text, file.name);
  };

  const handleTabSwitch = (tab: "file" | "extension") => {
    setSourceTab(tab);
    setExtFilter("all");
    setErrorMsg(null);
    const nextList = tab === "extension" ? extensionLinks : parsedLinks;
    setSelectedUrls(new Set(nextList.map((l) => l.url)));
  };

  const toggleUrl = (url: string) => {
    setSelectedUrls((prev) => {
      const next = new Set(prev);
      if (next.has(url)) next.delete(url);
      else next.add(url);
      return next;
    });
  };

  const allFilteredSelected =
    filteredLinks.length > 0 && filteredLinks.every((l) => selectedUrls.has(l.url));

  const toggleSelectAllFiltered = () => {
    setSelectedUrls((prev) => {
      const next = new Set(prev);
      if (allFilteredSelected) {
        filteredLinks.forEach((l) => next.delete(l.url));
      } else {
        filteredLinks.forEach((l) => next.add(l.url));
      }
      return next;
    });
  };

  const selectedLinks = activeLinks.filter((l) => selectedUrls.has(l.url));

  const handleQueue = async () => {
    if (selectedLinks.length === 0) return;
    await onQueueSelected(selectedLinks);
    onClose();
  };

  return (
    <AnimatePresence>
      <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
        <motion.div
          variants={backdropVariants}
          initial="initial"
          animate="animate"
          exit="exit"
          onClick={onClose}
          className="fixed inset-0 bg-black/60 backdrop-blur-sm"
        />

        <motion.div
          variants={modalVariants}
          initial="initial"
          animate="animate"
          exit="exit"
          className="relative w-full max-w-2xl rounded-2xl panel-3d shadow-2xl overflow-hidden flex flex-col z-10 border border-[var(--border-subtle)] max-h-[85vh]"
        >
          {/* Header */}
          <div className="px-5 py-4 border-b border-[var(--border-subtle)] flex items-center justify-between bg-[var(--bg-panel)]">
            <div className="flex items-center gap-2.5">
              <div className="p-2 rounded-xl bg-[var(--accent)]/15 text-[var(--accent)]">
                <Upload className="w-4 h-4" />
              </div>
              <div>
                <h3 className="text-sm font-semibold tracking-tight text-[var(--text-primary)]">
                  Batch Import Links
                </h3>
                <p className="text-xs text-[var(--text-muted)]">
                  Import from .txt, .csv, .html lists or browser page links
                </p>
              </div>
            </div>
            <button
              onClick={onClose}
              className="p-1 rounded-lg text-[var(--text-muted)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors"
            >
              <X className="w-4 h-4" />
            </button>
          </div>

          {/* Source Switcher */}
          <div className="px-5 pt-3 flex items-center justify-between gap-3">
            <div className="inline-flex rounded-xl p-1 bg-black/25 border border-[var(--border-subtle)]">
              <button
                type="button"
                onClick={() => handleTabSwitch("file")}
                className={`px-3 py-1.5 rounded-lg text-xs font-medium flex items-center gap-1.5 transition-colors ${
                  sourceTab === "file"
                    ? "bg-[var(--accent)] text-white"
                    : "text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
                }`}
              >
                <FileText className="w-3.5 h-3.5" />
                File / Paste
              </button>
              <button
                type="button"
                onClick={() => handleTabSwitch("extension")}
                className={`px-3 py-1.5 rounded-lg text-xs font-medium flex items-center gap-1.5 transition-colors ${
                  sourceTab === "extension"
                    ? "bg-[var(--accent)] text-white"
                    : "text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
                }`}
              >
                <Globe className="w-3.5 h-3.5" />
                Page Links ({extensionLinks.length})
              </button>
            </div>

            {sourceTab === "file" && (
              <div className="flex items-center gap-2">
                <Select
                  value={format}
                  onChange={(v) => setFormat(v as "auto" | ImportFormat)}
                  ariaLabel="Import file format"
                  className="w-36"
                  options={[
                    { value: "auto", label: "Auto-detect" },
                    { value: "txt", label: ".txt (Lines)" },
                    { value: "csv", label: ".csv (Table)" },
                    { value: "html", label: ".html (Markup)" },
                  ]}
                />
                <button
                  type="button"
                  onClick={() => fileInputRef.current?.click()}
                  className="px-3 py-1.5 rounded-lg text-xs font-medium border border-[var(--border-subtle)] text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors"
                >
                  Browse File
                </button>
                <input
                  ref={fileInputRef}
                  type="file"
                  accept=".txt,.csv,.html,.htm"
                  className="hidden"
                  onChange={(e) => {
                    const f = e.target.files?.[0];
                    if (f) void handleFile(f);
                  }}
                />
              </div>
            )}
          </div>

          {/* Body */}
          <div className="p-5 space-y-3 overflow-y-auto flex-1">
            {sourceTab === "file" && (
              <div
                onDragOver={(e) => {
                  e.preventDefault();
                  setIsDragging(true);
                }}
                onDragLeave={() => setIsDragging(false)}
                onDrop={(e) => {
                  e.preventDefault();
                  setIsDragging(false);
                  const f = e.dataTransfer.files?.[0];
                  if (f) void handleFile(f);
                }}
                className={`rounded-xl border transition-colors ${
                  isDragging
                    ? "border-[var(--accent)] bg-[var(--accent)]/10"
                    : "border-[var(--border-subtle)] bg-black/20"
                }`}
              >
                <textarea
                  rows={4}
                  value={rawText}
                  onChange={(e) => setRawText(e.target.value)}
                  placeholder="Drop a .txt, .csv, or .html file here — or paste URLs / CSV rows / HTML markup..."
                  className="w-full p-3 bg-transparent text-xs font-mono text-[var(--text-primary)] focus:outline-none resize-none leading-relaxed"
                />
                <div className="px-3 py-2 border-t border-[var(--border-subtle)] flex items-center justify-between">
                  <span className="text-[11px] text-[var(--text-muted)]">
                    Supports http://, https://, and magnet: links (max 5 MiB)
                  </span>
                  <button
                    type="button"
                    disabled={isParsing || !rawText.trim()}
                    onClick={() => void parseContent(rawText)}
                    className="px-3 py-1 rounded-lg text-xs font-semibold bg-[var(--accent)] text-white disabled:opacity-40 hover:opacity-90 transition-opacity"
                  >
                    {isParsing ? "Parsing..." : "Parse Links"}
                  </button>
                </div>
              </div>
            )}

            {errorMsg && (
              <div className="p-3 rounded-xl bg-rose-500/10 border border-rose-500/20 text-xs text-rose-400 flex items-center gap-2">
                <AlertCircle className="w-4 h-4 shrink-0" />
                <span>{errorMsg}</span>
              </div>
            )}

            {/* Extension Filter Pills + Select All */}
            {activeLinks.length > 0 && (
              <div className="space-y-2">
                <div className="flex flex-wrap items-center justify-between gap-2">
                  <div className="flex flex-wrap items-center gap-1.5">
                    <button
                      type="button"
                      onClick={() => setExtFilter("all")}
                      className={`px-2.5 py-1 rounded-lg text-[11px] font-medium transition-colors ${
                        extFilter === "all"
                          ? "bg-[var(--accent)] text-white"
                          : "bg-black/25 text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
                      }`}
                    >
                      All ({activeLinks.length})
                    </button>
                    {availableExtensions.map(([ext, count]) => (
                      <button
                        key={ext}
                        type="button"
                        onClick={() => setExtFilter(ext)}
                        className={`px-2.5 py-1 rounded-lg text-[11px] font-mono transition-colors ${
                          extFilter === ext
                            ? "bg-[var(--accent)] text-white"
                            : "bg-black/25 text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
                        }`}
                      >
                        {ext} ({count})
                      </button>
                    ))}
                  </div>

                  <button
                    type="button"
                    onClick={toggleSelectAllFiltered}
                    className="text-xs font-medium text-[var(--accent)] hover:underline flex items-center gap-1"
                  >
                    {allFilteredSelected ? (
                      <>
                        <CheckSquare className="w-3.5 h-3.5" /> Deselect All
                      </>
                    ) : (
                      <>
                        <Square className="w-3.5 h-3.5" /> Select All
                      </>
                    )}
                  </button>
                </div>

                {/* Links Checklist */}
                <div className="max-h-56 overflow-y-auto rounded-xl border border-[var(--border-subtle)] divide-y divide-[var(--border-subtle)] bg-black/20">
                  {filteredLinks.map((link) => {
                    const checked = selectedUrls.has(link.url);
                    return (
                      <label
                        key={link.url}
                        className="flex items-center gap-3 px-3 py-2 cursor-pointer hover:bg-[var(--bg-hover)] transition-colors"
                      >
                        <input
                          type="checkbox"
                          checked={checked}
                          onChange={() => toggleUrl(link.url)}
                          className="accent-[var(--accent)] rounded"
                        />
                        <div className="min-w-0 flex-1">
                          <div className="text-xs font-mono text-[var(--text-primary)] truncate">
                            {link.url}
                          </div>
                          {(link.filename_hint || link.category_hint) && (
                            <div className="text-[11px] text-[var(--text-muted)] truncate">
                              {[link.filename_hint, link.category_hint]
                                .filter(Boolean)
                                .join(" · ")}
                            </div>
                          )}
                        </div>
                      </label>
                    );
                  })}
                </div>
              </div>
            )}
          </div>

          {/* Footer */}
          <div className="px-5 py-3.5 border-t border-[var(--border-subtle)] bg-[var(--bg-panel)] flex items-center justify-between">
            <span className="text-xs text-[var(--text-muted)]">
              {selectedLinks.length} of {activeLinks.length} selected
            </span>
            <div className="flex items-center gap-2">
              <button
                type="button"
                onClick={onClose}
                className="px-3.5 py-1.5 rounded-xl text-xs font-medium text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors"
              >
                Cancel
              </button>
              <button
                type="button"
                disabled={selectedLinks.length === 0}
                onClick={() => void handleQueue()}
                className="px-4 py-1.5 rounded-xl text-xs font-semibold bg-[var(--accent)] text-white disabled:opacity-40 hover:opacity-90 transition-opacity flex items-center gap-1.5"
              >
                <Download className="w-3.5 h-3.5" />
                Queue Selected ({selectedLinks.length})
              </button>
            </div>
          </div>
        </motion.div>
      </div>
    </AnimatePresence>
  );
};

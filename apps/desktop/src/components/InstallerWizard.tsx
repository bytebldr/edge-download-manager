/**
 * InstallerWizard.tsx — EDM Custom Installer UI (Image 3)
 *
 * Rendered when ?mode=setup or #setup is detected, or when in installer mode.
 * Frameless dual-panel layout: left branding panel + right config panel.
 * Purple accent (#8B5CF6), dark chassis (#0D0F12), obsidian sidebar (#080A0D).
 */

import React, { useState, useEffect, useCallback } from "react";
import { openFolderDialog } from "../services/tauriBridge";
import {
  tauriWindowMinimize,
  tauriWindowClose,
  tauriStartDragging,
} from "../services/tauriBridge";

// ─── Types ───────────────────────────────────────────────────────────────────

interface InstallerWizardProps {
  /** Called after install completes or Cancel is pressed. */
  onDone: () => void;
}

type InstallPhase = "config" | "installing" | "done";

// ─── Component ───────────────────────────────────────────────────────────────

export const InstallerWizard: React.FC<InstallerWizardProps> = ({ onDone }) => {
  const [installPath, setInstallPath] = useState<string>(
    "C:\\Program Files\\EDM"
  );
  const [createShortcut, setCreateShortcut] = useState(true);
  const [startOnBoot, setStartOnBoot] = useState(true);
  const [browserIntegration, setBrowserIntegration] = useState(true);
  const [phase, setPhase] = useState<InstallPhase>("config");
  const [progress, setProgress] = useState(0);
  const [statusText, setStatusText] = useState("Preparing...");

  // Resolve default install path from env on mount
  useEffect(() => {
    const localAppData =
      (typeof process !== "undefined" && (process as NodeJS.Process & { env?: Record<string, string | undefined> }).env?.LOCALAPPDATA) || "";
    if (localAppData) {
      setInstallPath(`${localAppData}\\EDM`);
    }
  }, []);

  const handleBrowse = useCallback(async () => {
    try {
      const chosen = await openFolderDialog(installPath);
      if (chosen) setInstallPath(chosen);
    } catch {
      // dialog cancelled or unavailable
    }
  }, [installPath]);

  // Simulated install sequence (real hooks can be swapped in)
  const handleInstall = useCallback(async () => {
    setPhase("installing");
    const steps: [number, string][] = [
      [10, "Extracting files…"],
      [30, "Installing core engine…"],
      [55, "Registering native helpers…"],
      [70, createShortcut ? "Creating desktop shortcut…" : "Skipping shortcut…"],
      [85, startOnBoot ? "Adding startup entry…" : "Skipping startup entry…"],
      [95, browserIntegration ? "Registering browser integration…" : "Skipping browser integration…"],
      [100, "Installation complete!"],
    ];

    for (const [pct, msg] of steps) {
      await new Promise<void>((r) => setTimeout(r, 480));
      setProgress(pct);
      setStatusText(msg);
    }

    await new Promise<void>((r) => setTimeout(r, 600));
    setPhase("done");
  }, [createShortcut, startOnBoot, browserIntegration]);

  const handleLaunch = useCallback(() => {
    onDone();
  }, [onDone]);

  // ── Window drag on header ──────────────────────────────────────────────────
  const handleDragStart = useCallback(() => {
    try { tauriStartDragging(); } catch { /* non-Tauri fallback: no-op */ }
  }, []);

  // ── Render ─────────────────────────────────────────────────────────────────
  return (
    <div
      className="fixed inset-0 z-[9999] flex items-center justify-center bg-black/70 select-none"
      aria-modal="true"
      role="dialog"
      aria-labelledby="installer-title"
    >
      {/* Window shell */}
      <div
        className="relative flex overflow-hidden"
        style={{
          width: 680,
          height: 440,
          borderRadius: 12,
          background: "#0D0F12",
          border: "1px solid rgba(148,163,184,0.12)",
          boxShadow:
            "0 32px 80px rgba(0,0,0,0.75), 0 0 0 1px rgba(255,255,255,0.04) inset",
        }}
      >
        {/* ── Drag region (top) ── */}
        <div
          className="absolute top-0 left-0 right-0 h-10 z-10 cursor-default"
          onMouseDown={handleDragStart}
        />

        {/* ── Custom window controls (top-right) ── */}
        <div className="absolute top-3 right-3 z-20 flex items-center gap-1.5">
          <button
            onClick={() => { try { tauriWindowMinimize(); } catch { /* no-op */ } }}
            title="Minimize"
            className="w-7 h-7 flex items-center justify-center rounded-md text-[#5A5F6B] hover:text-[#B8BCC4] hover:bg-white/5 transition-colors text-sm leading-none cursor-pointer"
            style={{ fontFamily: "monospace" }}
          >
            —
          </button>
          <button
            onClick={() => { try { tauriWindowClose(); } catch { onDone(); } }}
            title="Close"
            className="w-7 h-7 flex items-center justify-center rounded-md text-[#5A5F6B] hover:text-red-400 hover:bg-red-500/10 transition-colors text-sm leading-none cursor-pointer"
          >
            ✕
          </button>
        </div>

        {/* ════════════════════════════════════════════════════════════════════
            LEFT PANEL — Obsidian Branding
            ════════════════════════════════════════════════════════════════════ */}
        <aside
          className="flex flex-col items-center justify-center gap-5 shrink-0"
          style={{
            width: 220,
            background: "#080A0D",
            borderRight: "1px solid rgba(148,163,184,0.08)",
          }}
        >
          {/* Logo */}
          <div
            className="rounded-2xl overflow-hidden flex items-center justify-center"
            style={{
              width: 72,
              height: 72,
              background: "rgba(139,92,246,0.1)",
              border: "1px solid rgba(139,92,246,0.25)",
              boxShadow: "0 0 24px rgba(139,92,246,0.15)",
            }}
          >
            <img
              src="/logo.png"
              alt="EDM Logo"
              className="w-16 h-16 object-contain"
              draggable={false}
              onError={(e) => {
                // Fallback: purple EDM wordmark if logo missing
                (e.currentTarget as HTMLImageElement).style.display = "none";
                const fallback = e.currentTarget.nextSibling as HTMLElement;
                if (fallback) fallback.style.display = "flex";
              }}
            />
            {/* Hidden fallback wordmark */}
            <span
              className="hidden text-2xl font-black text-[#8B5CF6] items-center justify-center w-full h-full"
            >
              EDM
            </span>
          </div>

          {/* Title */}
          <div className="text-center px-4">
            <h1
              id="installer-title"
              className="text-[22px] font-black tracking-tight text-white leading-none mb-2"
            >
              EDM
            </h1>
            <p
              className="text-[11px] text-[#6B7280] leading-snug"
              style={{ maxWidth: 160 }}
            >
              Fast, Intelligent Native Download Accelerator
            </p>
          </div>

          {/* Version badge */}
          <div
            className="px-3 py-1 text-[11px] font-semibold tracking-wide"
            style={{
              borderRadius: 999,
              background: "rgba(139,92,246,0.15)",
              border: "1px solid rgba(139,92,246,0.3)",
              color: "#A78BFA",
            }}
          >
            v0.0.13
          </div>
        </aside>

        {/* ════════════════════════════════════════════════════════════════════
            RIGHT PANEL — Config / Install / Done
            ════════════════════════════════════════════════════════════════════ */}
        <main className="flex-1 flex flex-col px-8 pt-12 pb-7 overflow-hidden">
          {/* ── CONFIG PHASE ─────────────────────────────────────────────── */}
          {phase === "config" && (
            <>
              <div className="mb-6">
                <h2 className="text-[17px] font-bold text-white tracking-tight mb-1">
                  Welcome to EDM Setup
                </h2>
                <p className="text-[12px] text-[#6B7280]">
                  Configure installation settings
                </p>
              </div>

              {/* Destination directory */}
              <div className="mb-5">
                <label className="block text-[11px] font-semibold text-[#9CA3AF] uppercase tracking-wider mb-2">
                  Install Location
                </label>
                <div className="flex gap-2">
                  <input
                    type="text"
                    value={installPath}
                    onChange={(e) => setInstallPath(e.target.value)}
                    spellCheck={false}
                    className="flex-1 h-9 px-3 text-[12.5px] text-[#D1D5DB] font-mono rounded-lg outline-none transition-colors"
                    style={{
                      background: "#0A0B0F",
                      border: "1px solid rgba(148,163,184,0.12)",
                      caretColor: "#8B5CF6",
                    }}
                    onFocus={(e) =>
                      (e.currentTarget.style.borderColor = "rgba(139,92,246,0.5)")
                    }
                    onBlur={(e) =>
                      (e.currentTarget.style.borderColor =
                        "rgba(148,163,184,0.12)")
                    }
                  />
                  <button
                    onClick={handleBrowse}
                    className="h-9 px-3.5 text-[12px] font-medium rounded-lg transition-colors cursor-pointer whitespace-nowrap"
                    style={{
                      background: "rgba(255,255,255,0.05)",
                      border: "1px solid rgba(148,163,184,0.14)",
                      color: "#9CA3AF",
                    }}
                    onMouseEnter={(e) => {
                      e.currentTarget.style.background = "rgba(255,255,255,0.09)";
                      e.currentTarget.style.color = "#D1D5DB";
                    }}
                    onMouseLeave={(e) => {
                      e.currentTarget.style.background = "rgba(255,255,255,0.05)";
                      e.currentTarget.style.color = "#9CA3AF";
                    }}
                  >
                    Browse…
                  </button>
                </div>
              </div>

              {/* Checkboxes */}
              <div className="space-y-3 mb-7">
                {(
                  [
                    {
                      id: "shortcut",
                      label: "Create Desktop shortcut",
                      value: createShortcut,
                      set: setCreateShortcut,
                    },
                    {
                      id: "boot",
                      label: "Start EDM on Windows boot",
                      value: startOnBoot,
                      set: setStartOnBoot,
                    },
                    {
                      id: "browser",
                      label: "Enable browser integration",
                      value: browserIntegration,
                      set: setBrowserIntegration,
                    },
                  ] as const
                ).map(({ id, label, value, set }) => (
                  <label
                    key={id}
                    htmlFor={`chk-${id}`}
                    className="flex items-center gap-2.5 cursor-pointer group select-none"
                  >
                    {/* Custom checkbox */}
                    <div
                      className="relative shrink-0 transition-all duration-150"
                      style={{
                        width: 16,
                        height: 16,
                        borderRadius: 4,
                        border: value
                          ? "1.5px solid #8B5CF6"
                          : "1.5px solid rgba(148,163,184,0.25)",
                        background: value
                          ? "#8B5CF6"
                          : "transparent",
                      }}
                    >
                      {value && (
                        <svg
                          className="absolute inset-0 m-auto"
                          width="10"
                          height="8"
                          viewBox="0 0 10 8"
                          fill="none"
                        >
                          <path
                            d="M1 4L3.5 6.5L9 1"
                            stroke="white"
                            strokeWidth="1.6"
                            strokeLinecap="round"
                            strokeLinejoin="round"
                          />
                        </svg>
                      )}
                    </div>
                    <input
                      id={`chk-${id}`}
                      type="checkbox"
                      checked={value}
                      onChange={(e) => set(e.target.checked)}
                      className="sr-only"
                    />
                    <span className="text-[12.5px] text-[#9CA3AF] group-hover:text-[#D1D5DB] transition-colors">
                      {label}
                    </span>
                  </label>
                ))}
              </div>

              {/* Action buttons */}
              <div className="mt-auto flex items-center gap-3">
                <button
                  onClick={onDone}
                  className="h-9 px-5 text-[12.5px] font-medium rounded-lg transition-colors cursor-pointer"
                  style={{
                    color: "#6B7280",
                    background: "transparent",
                    border: "1px solid rgba(148,163,184,0.1)",
                  }}
                  onMouseEnter={(e) => {
                    e.currentTarget.style.color = "#9CA3AF";
                    e.currentTarget.style.borderColor = "rgba(148,163,184,0.2)";
                  }}
                  onMouseLeave={(e) => {
                    e.currentTarget.style.color = "#6B7280";
                    e.currentTarget.style.borderColor =
                      "rgba(148,163,184,0.1)";
                  }}
                >
                  Cancel
                </button>

                <button
                  onClick={handleInstall}
                  className="flex-1 h-9 text-[13px] font-semibold rounded-lg text-white transition-all duration-150 cursor-pointer"
                  style={{
                    background:
                      "linear-gradient(135deg, #8B5CF6 0%, #7C3AED 100%)",
                    boxShadow:
                      "0 0 20px rgba(139,92,246,0.4), 0 2px 8px rgba(0,0,0,0.3), inset 0 1px 0 rgba(255,255,255,0.15)",
                  }}
                  onMouseEnter={(e) => {
                    e.currentTarget.style.boxShadow =
                      "0 0 28px rgba(139,92,246,0.55), 0 4px 12px rgba(0,0,0,0.3), inset 0 1px 0 rgba(255,255,255,0.15)";
                    e.currentTarget.style.transform = "translateY(-1px)";
                  }}
                  onMouseLeave={(e) => {
                    e.currentTarget.style.boxShadow =
                      "0 0 20px rgba(139,92,246,0.4), 0 2px 8px rgba(0,0,0,0.3), inset 0 1px 0 rgba(255,255,255,0.15)";
                    e.currentTarget.style.transform = "";
                  }}
                  onMouseDown={(e) => {
                    e.currentTarget.style.transform = "translateY(1px)";
                  }}
                  onMouseUp={(e) => {
                    e.currentTarget.style.transform = "translateY(-1px)";
                  }}
                >
                  Install Now
                </button>
              </div>
            </>
          )}

          {/* ── INSTALLING PHASE ─────────────────────────────────────────── */}
          {phase === "installing" && (
            <div className="flex flex-col h-full justify-center">
              <div className="mb-6">
                <h2 className="text-[17px] font-bold text-white tracking-tight mb-1">
                  Installing EDM
                </h2>
                <p className="text-[12px] text-[#6B7280]">
                  Please wait while EDM is being set up…
                </p>
              </div>

              {/* Progress bar */}
              <div
                className="w-full mb-3"
                style={{
                  height: 6,
                  borderRadius: 999,
                  background: "rgba(255,255,255,0.06)",
                }}
              >
                <div
                  style={{
                    height: "100%",
                    borderRadius: 999,
                    width: `${progress}%`,
                    background:
                      "linear-gradient(90deg, #7C3AED 0%, #8B5CF6 50%, #A78BFA 100%)",
                    boxShadow: "0 0 10px rgba(139,92,246,0.5)",
                    transition: "width 0.45s cubic-bezier(0.4,0,0.2,1)",
                  }}
                />
              </div>

              <div className="flex items-center justify-between">
                <span className="text-[11.5px] text-[#6B7280]">{statusText}</span>
                <span
                  className="text-[11.5px] font-semibold tabular-nums"
                  style={{ color: "#A78BFA" }}
                >
                  {progress}%
                </span>
              </div>

              {/* Animated dots */}
              <div className="mt-8 flex items-center gap-1.5">
                {[0, 1, 2].map((i) => (
                  <div
                    key={i}
                    style={{
                      width: 6,
                      height: 6,
                      borderRadius: "50%",
                      background: "#8B5CF6",
                      animation: `installer-pulse 1.2s ease-in-out ${i * 0.2}s infinite`,
                      opacity: 0.3,
                    }}
                  />
                ))}
                <style>{`
                  @keyframes installer-pulse {
                    0%, 100% { opacity: 0.25; transform: scale(0.85); }
                    50% { opacity: 1; transform: scale(1.15); }
                  }
                `}</style>
              </div>
            </div>
          )}

          {/* ── DONE PHASE ───────────────────────────────────────────────── */}
          {phase === "done" && (
            <div className="flex flex-col h-full justify-center">
              {/* Success checkmark */}
              <div className="mb-5">
                <div
                  className="inline-flex items-center justify-center mb-4"
                  style={{
                    width: 48,
                    height: 48,
                    borderRadius: "50%",
                    background: "rgba(139,92,246,0.15)",
                    border: "1.5px solid rgba(139,92,246,0.4)",
                    boxShadow: "0 0 20px rgba(139,92,246,0.2)",
                  }}
                >
                  <svg width="22" height="17" viewBox="0 0 22 17" fill="none">
                    <path
                      d="M1.5 8.5L7.5 14.5L20.5 1.5"
                      stroke="#A78BFA"
                      strokeWidth="2.2"
                      strokeLinecap="round"
                      strokeLinejoin="round"
                    />
                  </svg>
                </div>
                <h2 className="text-[17px] font-bold text-white tracking-tight mb-1">
                  Installation Complete
                </h2>
                <p className="text-[12px] text-[#6B7280]">
                  EDM has been installed to{" "}
                  <span className="text-[#9CA3AF] font-mono text-[11px]">
                    {installPath}
                  </span>
                </p>
              </div>

              <button
                onClick={handleLaunch}
                className="mt-auto h-9 w-full text-[13px] font-semibold rounded-lg text-white cursor-pointer"
                style={{
                  background:
                    "linear-gradient(135deg, #8B5CF6 0%, #7C3AED 100%)",
                  boxShadow:
                    "0 0 20px rgba(139,92,246,0.4), 0 2px 8px rgba(0,0,0,0.3), inset 0 1px 0 rgba(255,255,255,0.15)",
                }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.boxShadow =
                    "0 0 28px rgba(139,92,246,0.55), 0 4px 12px rgba(0,0,0,0.3), inset 0 1px 0 rgba(255,255,255,0.15)";
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.boxShadow =
                    "0 0 20px rgba(139,92,246,0.4), 0 2px 8px rgba(0,0,0,0.3), inset 0 1px 0 rgba(255,255,255,0.15)";
                }}
              >
                Launch EDM →
              </button>
            </div>
          )}
        </main>
      </div>
    </div>
  );
};

export default InstallerWizard;

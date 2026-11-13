/**
 * UninstallerWizard.tsx — EDM Custom Uninstaller UI
 *
 * Rendered when ?mode=uninstall or #uninstall is detected.
 * Frameless dual-panel layout matching InstallerWizard (680×440).
 * Obsidian sidebar (#080A0D), dark chassis (#0D0F12), rose/crimson accents.
 */

import React, { useState, useCallback } from "react";
import {
  tauriWindowMinimize,
  tauriWindowClose,
  tauriStartDragging,
  tauriPerformNativeUninstallation,
} from "../services/tauriBridge";

interface UninstallerWizardProps {
  onDone: () => void;
}

type UninstallPhase = "confirm" | "uninstalling" | "done";

export const UninstallerWizard: React.FC<UninstallerWizardProps> = ({ onDone }) => {
  const [purgeData, setPurgeData] = useState(false);
  const [phase, setPhase] = useState<UninstallPhase>("confirm");
  const [progress, setProgress] = useState(0);
  const [statusText, setStatusText] = useState("Preparing...");
  const [errorText, setErrorText] = useState<string | null>(null);

  const handleUninstall = useCallback(async () => {
    setPhase("uninstalling");
    setErrorText(null);

    const steps: [number, string][] = [
      [15, "Terminating background processes…"],
      [35, "Removing Desktop and Start Menu shortcuts…"],
      [55, "Unregistering browser integration and native host…"],
      [75, "Cleaning registry and startup entries…"],
      [90, purgeData ? "Purging database and settings…" : "Preserving user download files…"],
    ];

    for (const [pct, msg] of steps) {
      await new Promise<void>((r) => setTimeout(r, 300));
      setProgress(pct);
      setStatusText(msg);
    }

    try {
      await tauriPerformNativeUninstallation({
        purgeUserData: purgeData,
      });
      setProgress(100);
      setStatusText("Uninstallation complete!");
      await new Promise<void>((r) => setTimeout(r, 400));
      setPhase("done");
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      setErrorText(msg || "Uninstallation failed.");
      setProgress(0);
      setStatusText("Error");
      setPhase("confirm");
    }
  }, [purgeData]);

  const handleClose = useCallback(() => {
    onDone();
    tauriWindowClose();
  }, [onDone]);

  return (
    <div
      className="fixed inset-0 flex select-none overflow-hidden font-sans"
      style={{
        background: "#0D0F12",
        color: "#E5E7EB",
        boxShadow: "0 25px 60px rgba(0,0,0,0.85), 0 0 0 1px rgba(244,63,94,0.15)",
        borderRadius: 12,
      }}
    >
      {/* ── Title bar / drag area ────────────────────────────────────────── */}
      <div
        onMouseDown={tauriStartDragging}
        className="absolute top-0 left-0 right-0 h-10 flex items-center justify-end px-3 z-50 cursor-grab active:cursor-grabbing"
        style={{ WebkitAppRegion: "drag" } as React.CSSProperties}
      >
        <div
          className="flex items-center gap-1.5"
          style={{ WebkitAppRegion: "no-drag" } as React.CSSProperties}
        >
          <button
            onClick={tauriWindowMinimize}
            aria-label="Minimize"
            className="w-7 h-7 flex items-center justify-center rounded-md transition-colors cursor-pointer text-[#6B7280] hover:text-white hover:bg-white/5"
          >
            <svg width="10" height="1" viewBox="0 0 10 1" fill="currentColor">
              <rect width="10" height="1" rx="0.5" />
            </svg>
          </button>
          <button
            onClick={handleClose}
            aria-label="Close"
            className="w-7 h-7 flex items-center justify-center rounded-md transition-colors cursor-pointer text-[#6B7280] hover:text-white hover:bg-red-500/20"
          >
            <svg width="9" height="9" viewBox="0 0 9 9" fill="none">
              <path
                d="M1 1L8 8M8 1L1 8"
                stroke="currentColor"
                strokeWidth="1.3"
                strokeLinecap="round"
              />
            </svg>
          </button>
        </div>
      </div>

      {/* ── Layout container ─────────────────────────────────────────────── */}
      <div className="flex w-full h-full">
        {/* ════════════════════════════════════════════════════════════════════
            LEFT SIDEBAR — Branding Panel
            ════════════════════════════════════════════════════════════════════ */}
        <aside
          onMouseDown={tauriStartDragging}
          className="flex flex-col items-center justify-center cursor-grab active:cursor-grabbing relative"
          style={{
            width: 220,
            background: "#080A0D",
            borderRight: "1px solid rgba(148,163,184,0.08)",
            gap: 16,
            WebkitAppRegion: "drag",
          } as React.CSSProperties}
        >
          {/* Subtle gradient orb */}
          <div
            className="absolute pointer-events-none"
            style={{
              width: 140,
              height: 140,
              borderRadius: "50%",
              background:
                "radial-gradient(circle, rgba(244,63,94,0.12) 0%, transparent 70%)",
              top: "20%",
              left: "50%",
              transform: "translateX(-50%)",
            }}
          />

          {/* Logo container */}
          <div
            className="rounded-2xl overflow-hidden flex items-center justify-center"
            style={{
              width: 72,
              height: 72,
              background: "rgba(244,63,94,0.08)",
              border: "1px solid rgba(244,63,94,0.25)",
              boxShadow: "0 0 24px rgba(244,63,94,0.12)",
            }}
          >
            <img
              src="/logo.png"
              alt="EDM Logo"
              className="w-16 h-16 object-contain"
              draggable={false}
              onError={(e) => {
                (e.currentTarget as HTMLImageElement).style.display = "none";
                const fallback = e.currentTarget.nextSibling as HTMLElement;
                if (fallback) fallback.style.display = "flex";
              }}
            />
            <span className="hidden text-2xl font-black text-[#F43F5E] items-center justify-center w-full h-full">
              EDM
            </span>
          </div>

          {/* App title & tagline */}
          <div className="text-center px-4">
            <h1 className="text-[22px] font-black tracking-tight text-white leading-none mb-2">
              EDM
            </h1>
            <p className="text-[11px] text-[#6B7280] leading-snug" style={{ maxWidth: 160 }}>
              Fast, Intelligent Native Download Accelerator
            </p>
          </div>

          {/* Version + uninstaller badge */}
          <div className="flex items-center gap-1.5">
            <span
              className="px-2.5 py-0.5 text-[10.5px] font-semibold tracking-wide rounded-full"
              style={{
                background: "rgba(148,163,184,0.1)",
                border: "1px solid rgba(148,163,184,0.2)",
                color: "#9CA3AF",
              }}
            >
              v0.0.13
            </span>
            <span
              className="px-2.5 py-0.5 text-[10.5px] font-semibold tracking-wide rounded-full"
              style={{
                background: "rgba(244,63,94,0.15)",
                border: "1px solid rgba(244,63,94,0.3)",
                color: "#FB7185",
              }}
            >
              Uninstall
            </span>
          </div>
        </aside>

        {/* ════════════════════════════════════════════════════════════════════
            RIGHT PANEL — Confirm / Uninstalling / Done
            ════════════════════════════════════════════════════════════════════ */}
        <main className="flex-1 flex flex-col px-8 pt-12 pb-7 overflow-hidden">
          {/* ── CONFIRM PHASE ─────────────────────────────────────────────── */}
          {phase === "confirm" && (
            <>
              <div className="mb-6">
                <h2 className="text-[17px] font-bold text-white tracking-tight mb-1">
                  Uninstall EDM
                </h2>
                <p className="text-[12px] text-[#6B7280]">
                  Are you sure you want to completely remove EDM from your computer?
                </p>
              </div>

              {/* Options */}
              <div className="space-y-3 mb-6">
                <label className="flex items-start gap-3 cursor-not-allowed select-none opacity-80">
                  <div
                    className="w-4 h-4 mt-0.5 rounded flex items-center justify-center shrink-0"
                    style={{
                      background: "#F43F5E",
                      border: "1px solid #F43F5E",
                    }}
                  >
                    <svg width="10" height="8" viewBox="0 0 10 8" fill="none">
                      <path
                        d="M1 4L3.5 6.5L9 1"
                        stroke="white"
                        strokeWidth="1.6"
                        strokeLinecap="round"
                        strokeLinejoin="round"
                      />
                    </svg>
                  </div>
                  <div>
                    <span className="text-[12.5px] text-[#D1D5DB] font-medium block">
                      Remove application files, shortcuts & startup entries
                    </span>
                    <span className="text-[11px] text-[#6B7280]">
                      Deletes binaries from install location and clears Windows shortcuts
                    </span>
                  </div>
                </label>

                <label
                  onClick={() => setPurgeData(!purgeData)}
                  className="flex items-start gap-3 cursor-pointer select-none group"
                >
                  <div
                    className="w-4 h-4 mt-0.5 rounded flex items-center justify-center shrink-0 transition-colors"
                    style={{
                      background: purgeData ? "#F43F5E" : "#0A0B0F",
                      border: purgeData
                        ? "1px solid #F43F5E"
                        : "1px solid rgba(148,163,184,0.3)",
                    }}
                  >
                    {purgeData && (
                      <svg width="10" height="8" viewBox="0 0 10 8" fill="none">
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
                  <div>
                    <span className="text-[12.5px] text-[#D1D5DB] font-medium block group-hover:text-white transition-colors">
                      Also delete database, download history and settings
                    </span>
                    <span className="text-[11px] text-[#9CA3AF]">
                      Clears SQLite database, queue state, and categories (cannot be undone)
                    </span>
                  </div>
                </label>
              </div>

              {errorText && (
                <div
                  className="mb-4 px-3 py-2 rounded-lg text-[11.5px] text-red-300"
                  style={{
                    background: "rgba(239,68,68,0.08)",
                    border: "1px solid rgba(239,68,68,0.25)",
                  }}
                >
                  ⚠ {errorText}
                </div>
              )}

              {/* Action buttons */}
              <div className="mt-auto flex items-center gap-3">
                <button
                  onClick={handleClose}
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
                    e.currentTarget.style.borderColor = "rgba(148,163,184,0.1)";
                  }}
                >
                  Cancel
                </button>

                <button
                  onClick={handleUninstall}
                  className="flex-1 h-9 text-[13px] font-semibold rounded-lg text-white transition-all duration-150 cursor-pointer"
                  style={{
                    background: "linear-gradient(135deg, #F43F5E 0%, #E11D48 100%)",
                    boxShadow:
                      "0 0 20px rgba(244,63,94,0.4), 0 2px 8px rgba(0,0,0,0.3), inset 0 1px 0 rgba(255,255,255,0.15)",
                  }}
                  onMouseEnter={(e) => {
                    e.currentTarget.style.boxShadow =
                      "0 0 28px rgba(244,63,94,0.55), 0 4px 12px rgba(0,0,0,0.3), inset 0 1px 0 rgba(255,255,255,0.15)";
                    e.currentTarget.style.transform = "translateY(-1px)";
                  }}
                  onMouseLeave={(e) => {
                    e.currentTarget.style.boxShadow =
                      "0 0 20px rgba(244,63,94,0.4), 0 2px 8px rgba(0,0,0,0.3), inset 0 1px 0 rgba(255,255,255,0.15)";
                    e.currentTarget.style.transform = "";
                  }}
                >
                  Uninstall Now
                </button>
              </div>
            </>
          )}

          {/* ── UNINSTALLING PHASE ────────────────────────────────────────── */}
          {phase === "uninstalling" && (
            <div className="flex flex-col h-full justify-center">
              <div className="mb-6">
                <h2 className="text-[17px] font-bold text-white tracking-tight mb-1">
                  Uninstalling EDM
                </h2>
                <p className="text-[12px] text-[#6B7280]">
                  Please wait while EDM is being removed from your system…
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
                    width: `${progress}%`,
                    borderRadius: 999,
                    background: "linear-gradient(90deg, #F43F5E, #FB7185)",
                    boxShadow: "0 0 12px rgba(244,63,94,0.6)",
                    transition: "width 0.35s ease-out",
                  }}
                />
              </div>

              <div className="flex items-center justify-between text-[11.5px] text-[#6B7280]">
                <span>{statusText}</span>
                <span className="font-mono text-[#9CA3AF]">{progress}%</span>
              </div>
            </div>
          )}

          {/* ── DONE PHASE ───────────────────────────────────────────────── */}
          {phase === "done" && (
            <div className="flex flex-col h-full justify-center">
              <div className="mb-5">
                <div
                  className="inline-flex items-center justify-center mb-4"
                  style={{
                    width: 48,
                    height: 48,
                    borderRadius: "50%",
                    background: "rgba(244,63,94,0.15)",
                    border: "1.5px solid rgba(244,63,94,0.4)",
                    boxShadow: "0 0 20px rgba(244,63,94,0.2)",
                  }}
                >
                  <svg width="22" height="17" viewBox="0 0 22 17" fill="none">
                    <path
                      d="M1.5 8.5L7.5 14.5L20.5 1.5"
                      stroke="#FB7185"
                      strokeWidth="2.2"
                      strokeLinecap="round"
                      strokeLinejoin="round"
                    />
                  </svg>
                </div>
                <h2 className="text-[17px] font-bold text-white tracking-tight mb-1">
                  Uninstallation Complete
                </h2>
                <p className="text-[12px] text-[#6B7280]">
                  EDM has been completely removed from your computer. Thank you for trying EDM.
                </p>
              </div>

              <button
                onClick={handleClose}
                className="mt-auto h-9 w-full text-[13px] font-semibold rounded-lg text-white cursor-pointer"
                style={{
                  background: "linear-gradient(135deg, #F43F5E 0%, #E11D48 100%)",
                  boxShadow:
                    "0 0 20px rgba(244,63,94,0.4), 0 2px 8px rgba(0,0,0,0.3), inset 0 1px 0 rgba(255,255,255,0.15)",
                }}
              >
                Close
              </button>
            </div>
          )}
        </main>
      </div>
    </div>
  );
};

export default UninstallerWizard;

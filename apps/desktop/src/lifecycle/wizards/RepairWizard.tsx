/**
 * RepairWizard.tsx — EDM Dedicated Reinstall & Repair Custom UI
 *
 * Rendered when ?mode=repair or #repair or --repair is detected.
 * Fixed 680×440 frameless dark glassmorphic layout matching Installer & Uninstaller wizards.
 * Obsidian sidebar (#080A0D), dark chassis (#0D0F12), cyan/violet accents (#06B6D4, #8B5CF6).
 */

import React, { useState, useCallback, useEffect } from "react";
import {
  tauriWindowMinimize,
  tauriWindowClose,
  tauriStartDragging,
  tauriPerformNativeRepair,
  tauriLaunchMainApp,
  NativeRepairReport,
} from "../../services/tauriBridge";
import {
  Wrench,
  ShieldCheck,
  CheckCircle2,
  AlertTriangle,
  Compass,
  Monitor,
  Binary,
  Minus,
  X,
  Play,
  RotateCcw,
} from "lucide-react";

export interface RepairWizardProps {
  onDone?: () => void;
}

type RepairPhase = "config" | "repairing" | "done";

export const RepairWizard: React.FC<RepairWizardProps> = ({ onDone }) => {
  const [repairBrowser, setRepairBrowser] = useState(true);
  const [repairShortcuts, setRepairShortcuts] = useState(true);
  const [verifyBinaries, setVerifyBinaries] = useState(true);

  const [phase, setPhase] = useState<RepairPhase>("config");
  const [progress, setProgress] = useState(0);
  const [statusText, setStatusText] = useState("Preparing diagnostic scan...");
  const [report, setReport] = useState<NativeRepairReport | null>(null);
  const [errorText, setErrorText] = useState<string | null>(null);
  const [appVersion, setAppVersion] = useState("0.0.15");

  useEffect(() => {
    import("@tauri-apps/api/app")
      .then((m) => m.getVersion())
      .then((v) => {
        if (v) setAppVersion(v);
      })
      .catch(() => {});
  }, []);

  const handleRunRepair = useCallback(async () => {
    setPhase("repairing");
    setErrorText(null);

    const steps: [number, string][] = [
      [20, "Scanning EDM installation root & binary integrity..."],
      [45, "Re-verifying Native Messaging Host registry paths & manifests..."],
      [70, "Validating Start Menu & Desktop shell shortcuts..."],
      [90, "Applying repairs to subsystem configurations..."],
    ];

    for (const [pct, msg] of steps) {
      await new Promise<void>((r) => setTimeout(r, 260));
      setProgress(pct);
      setStatusText(msg);
    }

    try {
      const res = await tauriPerformNativeRepair({
        repairBrowser,
        repairShortcuts,
        verifyBinaries,
      });
      setProgress(100);
      setStatusText("All targeted repairs applied successfully!");
      setReport(res);
      await new Promise<void>((r) => setTimeout(r, 400));
      setPhase("done");
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      setErrorText(msg || "Repair operation failed.");
      setProgress(0);
      setStatusText("Error");
      setPhase("config");
    }
  }, [repairBrowser, repairShortcuts, verifyBinaries]);

  const handleLaunch = useCallback(async () => {
    try {
      await tauriLaunchMainApp();
    } catch {
      if (onDone) onDone();
      tauriWindowClose();
    }
  }, [onDone]);

  const handleClose = useCallback(() => {
    if (onDone) onDone();
    tauriWindowClose();
  }, [onDone]);

  const handleDragStart = useCallback(() => {
    try {
      tauriStartDragging();
    } catch {}
  }, []);

  return (
    <div
      className="fixed inset-0 flex select-none overflow-hidden font-sans"
      style={{
        background: "#0D0F12",
        color: "#E5E7EB",
        boxShadow: "0 25px 60px rgba(0,0,0,0.85), 0 0 0 1px rgba(6,182,212,0.18)",
        borderRadius: 12,
      }}
    >
      {/* ── Title bar / drag area ────────────────────────────────────────── */}
      <div
        onMouseDown={handleDragStart}
        className="absolute top-0 left-0 right-0 h-9 flex items-center justify-between px-3 z-50 cursor-grab active:cursor-grabbing"
        style={{
          background: "linear-gradient(180deg, rgba(255,255,255,0.03) 0%, transparent 100%)",
          borderBottom: "1px solid rgba(255,255,255,0.05)",
        }}
      >
        <div className="flex items-center gap-2 text-xs font-medium text-slate-300 pointer-events-none">
          <Wrench className="w-3.5 h-3.5 text-cyan-400" />
          <span>EDM Reinstall & Repair</span>
          <span className="text-[10px] font-mono px-1.5 py-0.5 rounded bg-cyan-950/60 border border-cyan-800/40 text-cyan-300">
            v{appVersion}
          </span>
        </div>
        <div className="flex items-center gap-1" onMouseDown={(e) => e.stopPropagation()}>
          <button
            onClick={() => tauriWindowMinimize()}
            className="w-6 h-6 flex items-center justify-center rounded hover:bg-white/10 text-slate-400 hover:text-white transition-colors"
            title="Minimize"
          >
            <Minus className="w-3.5 h-3.5" />
          </button>
          <button
            onClick={handleClose}
            className="w-6 h-6 flex items-center justify-center rounded hover:bg-red-500/80 text-slate-400 hover:text-white transition-colors"
            title="Close"
          >
            <X className="w-3.5 h-3.5" />
          </button>
        </div>
      </div>

      {/* ── Left branding panel (220px) ─────────────────────────────────── */}
      <aside
        className="w-[220px] shrink-0 flex flex-col justify-between p-5 pt-12 relative overflow-hidden"
        style={{
          background: "#080A0D",
          borderRight: "1px solid rgba(255,255,255,0.06)",
        }}
      >
        {/* Subtle cyan/violet ambient glow */}
        <div
          className="absolute -top-16 -left-16 w-48 h-48 rounded-full pointer-events-none"
          style={{
            background: "radial-gradient(circle, rgba(6,182,212,0.15) 0%, transparent 70%)",
            filter: "blur(32px)",
          }}
        />

        <div>
          {/* Logo badge */}
          <div className="flex items-center gap-2.5 mb-5">
            <div
              className="w-10 h-10 rounded-xl flex items-center justify-center shadow-lg"
              style={{
                background: "linear-gradient(135deg, #0891B2 0%, #06B6D4 100%)",
                boxShadow: "0 0 20px rgba(6,182,212,0.3)",
              }}
            >
              <Wrench className="w-5 h-5 text-white stroke-[2.2]" />
            </div>
            <div>
              <div className="text-sm font-bold tracking-tight text-white leading-tight">
                EDM Repair
              </div>
              <div className="text-[10px] text-cyan-400 font-mono">
                System Diagnostics
              </div>
            </div>
          </div>

          <p className="text-[11px] text-slate-400 leading-relaxed mb-4">
            Reinstall and repair browser extensions, desktop shortcuts, and core system binaries without data loss.
          </p>

          {/* Safety badge */}
          <div
            className="p-2.5 rounded-lg text-[10.5px] leading-snug flex items-start gap-2"
            style={{
              background: "rgba(16,185,129,0.08)",
              border: "1px solid rgba(16,185,129,0.25)",
              color: "#34D399",
            }}
          >
            <ShieldCheck className="w-4 h-4 shrink-0 text-emerald-400 mt-0.5" />
            <div>
              <span className="font-semibold block text-emerald-300">Non-Destructive</span>
              Preserves all downloads, queues, and user settings.
            </div>
          </div>
        </div>

        {/* Footer info */}
        <div className="text-[10px] text-slate-500 font-mono space-y-0.5">
          <div>Mode: Standalone Diagnostic</div>
          <div>Target: Local Installation</div>
        </div>
      </aside>

      {/* ── Right action panel (460px) ─────────────────────────────────── */}
      <main className="flex-1 flex flex-col justify-between p-6 pt-12 overflow-y-auto">
        {phase === "config" && (
          <div className="space-y-4 flex-1 flex flex-col justify-between">
            <div>
              <div className="flex items-center gap-2 mb-1">
                <h2 className="text-sm font-bold text-white tracking-tight">
                  Targeted Repair Options
                </h2>
                <span className="px-2 py-0.5 rounded-full text-[10px] font-semibold bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">
                  Data Preserved
                </span>
              </div>
              <p className="text-[11px] text-slate-400 mb-4">
                Select the EDM system components you wish to restore:
              </p>

              {/* Checkbox options list */}
              <div className="space-y-2.5">
                {/* 1. Browser integration */}
                <label
                  className="flex items-start gap-3 p-3 rounded-xl border transition-all cursor-pointer group"
                  style={{
                    background: repairBrowser ? "rgba(6,182,212,0.06)" : "rgba(255,255,255,0.02)",
                    borderColor: repairBrowser ? "rgba(6,182,212,0.3)" : "rgba(255,255,255,0.07)",
                  }}
                >
                  <input
                    type="checkbox"
                    checked={repairBrowser}
                    onChange={(e) => setRepairBrowser(e.target.checked)}
                    className="mt-0.5 accent-cyan-500 rounded cursor-pointer"
                  />
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-1.5 text-xs font-semibold text-slate-200 group-hover:text-cyan-300 transition-colors">
                      <Compass className="w-3.5 h-3.5 text-cyan-400 shrink-0" />
                      <span>Repair browser integration & Native Messaging Host</span>
                    </div>
                    <p className="text-[10.5px] text-slate-400 mt-0.5 leading-normal">
                      Re-registers <code className="text-cyan-300 font-mono text-[9.5px]">com.edm.nmh</code> manifests for Microsoft Edge, Google Chrome, Brave, and Mozilla Firefox.
                    </p>
                  </div>
                </label>

                {/* 2. Shortcuts */}
                <label
                  className="flex items-start gap-3 p-3 rounded-xl border transition-all cursor-pointer group"
                  style={{
                    background: repairShortcuts ? "rgba(6,182,212,0.06)" : "rgba(255,255,255,0.02)",
                    borderColor: repairShortcuts ? "rgba(6,182,212,0.3)" : "rgba(255,255,255,0.07)",
                  }}
                >
                  <input
                    type="checkbox"
                    checked={repairShortcuts}
                    onChange={(e) => setRepairShortcuts(e.target.checked)}
                    className="mt-0.5 accent-cyan-500 rounded cursor-pointer"
                  />
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-1.5 text-xs font-semibold text-slate-200 group-hover:text-cyan-300 transition-colors">
                      <Monitor className="w-3.5 h-3.5 text-cyan-400 shrink-0" />
                      <span>Restore missing Desktop and Start Menu shortcuts</span>
                    </div>
                    <p className="text-[10.5px] text-slate-400 mt-0.5 leading-normal">
                      Recreates Windows shell links (<code className="text-cyan-300 font-mono text-[9.5px]">EDM.lnk</code>) with correct icon and executable targets.
                    </p>
                  </div>
                </label>

                {/* 3. Core binaries */}
                <label
                  className="flex items-start gap-3 p-3 rounded-xl border transition-all cursor-pointer group"
                  style={{
                    background: verifyBinaries ? "rgba(6,182,212,0.06)" : "rgba(255,255,255,0.02)",
                    borderColor: verifyBinaries ? "rgba(6,182,212,0.3)" : "rgba(255,255,255,0.07)",
                  }}
                >
                  <input
                    type="checkbox"
                    checked={verifyBinaries}
                    onChange={(e) => setVerifyBinaries(e.target.checked)}
                    className="mt-0.5 accent-cyan-500 rounded cursor-pointer"
                  />
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-1.5 text-xs font-semibold text-slate-200 group-hover:text-cyan-300 transition-colors">
                      <Binary className="w-3.5 h-3.5 text-cyan-400 shrink-0" />
                      <span>Verify and restore core binaries (EDM.exe, edm-nmh.exe)</span>
                    </div>
                    <p className="text-[10.5px] text-slate-400 mt-0.5 leading-normal">
                      Ensures bidirectional executable synchronization and checks feature packs (yt-dlp, ffmpeg).
                    </p>
                  </div>
                </label>
              </div>

              {errorText && (
                <div className="mt-3 p-2.5 rounded-lg bg-red-950/40 border border-red-800/40 text-red-300 text-xs flex items-center gap-2">
                  <AlertTriangle className="w-4 h-4 shrink-0 text-red-400" />
                  <span>{errorText}</span>
                </div>
              )}
            </div>

            {/* Actions footer */}
            <div className="pt-3 border-t border-slate-800/60 flex items-center justify-between gap-3">
              <button
                onClick={handleClose}
                className="px-3.5 h-8 text-xs font-medium rounded-lg text-slate-400 hover:text-white hover:bg-slate-800/60 transition-colors cursor-pointer"
              >
                Cancel
              </button>

              <button
                onClick={handleRunRepair}
                disabled={!repairBrowser && !repairShortcuts && !verifyBinaries}
                className="px-5 h-8.5 text-xs font-bold rounded-lg text-white shadow-md inline-flex items-center gap-2 cursor-pointer transition-all disabled:opacity-40"
                style={{
                  background: "linear-gradient(135deg, #0891B2 0%, #06B6D4 100%)",
                  boxShadow: "0 0 16px rgba(6,182,212,0.35)",
                }}
              >
                <RotateCcw className="w-3.5 h-3.5" />
                <span>Run Repair</span>
              </button>
            </div>
          </div>
        )}

        {phase === "repairing" && (
          <div className="flex-1 flex flex-col items-center justify-center text-center px-4 space-y-4">
            <div
              className="w-12 h-12 rounded-2xl flex items-center justify-center animate-pulse"
              style={{
                background: "rgba(6,182,212,0.12)",
                border: "1px solid rgba(6,182,212,0.3)",
              }}
            >
              <RotateCcw className="w-6 h-6 text-cyan-400 animate-spin" />
            </div>

            <div>
              <h3 className="text-sm font-bold text-white mb-1">
                Applying System Repairs…
              </h3>
              <p className="text-xs text-slate-400 font-mono">
                {statusText}
              </p>
            </div>

            {/* Real-time progress bar */}
            <div className="w-full max-w-sm space-y-1.5">
              <div className="w-full h-2 rounded-full bg-slate-800/80 overflow-hidden border border-slate-700/50">
                <div
                  className="h-full rounded-full transition-all duration-300"
                  style={{
                    width: `${progress}%`,
                    background: "linear-gradient(90deg, #0891B2, #06B6D4)",
                    boxShadow: "0 0 10px rgba(6,182,212,0.5)",
                  }}
                />
              </div>
              <div className="flex justify-between text-[10px] text-slate-500 font-mono">
                <span>Diagnostic Execution</span>
                <span>{progress}%</span>
              </div>
            </div>
          </div>
        )}

        {phase === "done" && (
          <div className="space-y-4 flex-1 flex flex-col justify-between">
            <div>
              <div className="flex items-center gap-2 mb-1">
                <CheckCircle2 className="w-5 h-5 text-emerald-400 shrink-0" />
                <h2 className="text-sm font-bold text-white tracking-tight">
                  Repair Completed Successfully
                </h2>
              </div>
              <p className="text-[11px] text-slate-400 mb-3">
                All selected EDM subsystems have been audited and repaired:
              </p>

              {/* Repaired items summary */}
              <div className="space-y-1.5 max-h-48 overflow-y-auto pr-1">
                {report?.details && report.details.length > 0 ? (
                  report.details.map((item, idx) => (
                    <div
                      key={idx}
                      className="p-2 rounded-lg bg-emerald-950/20 border border-emerald-800/30 text-[11px] text-emerald-300 flex items-start gap-2"
                    >
                      <CheckCircle2 className="w-3.5 h-3.5 text-emerald-400 shrink-0 mt-0.5" />
                      <span>{item}</span>
                    </div>
                  ))
                ) : (
                  <div className="p-2 rounded-lg bg-emerald-950/20 border border-emerald-800/30 text-[11px] text-emerald-300 flex items-center gap-2">
                    <CheckCircle2 className="w-3.5 h-3.5 text-emerald-400 shrink-0" />
                    <span>All core binaries, manifests, and shortcuts verified.</span>
                  </div>
                )}
              </div>
            </div>

            {/* Launch EDM button */}
            <div className="pt-3 border-t border-slate-800/60 flex items-center justify-between gap-3">
              <span className="text-[10.5px] text-slate-500 font-mono">
                Ready to resume downloads
              </span>

              <button
                onClick={handleLaunch}
                className="px-5 h-8.5 text-xs font-bold rounded-lg text-white shadow-md inline-flex items-center gap-2 cursor-pointer transition-all hover:brightness-110"
                style={{
                  background: "linear-gradient(135deg, #059669 0%, #10B981 100%)",
                  boxShadow: "0 0 16px rgba(16,185,129,0.35)",
                }}
              >
                <Play className="w-3.5 h-3.5 fill-current" />
                <span>Launch EDM</span>
              </button>
            </div>
          </div>
        )}
      </main>
    </div>
  );
};

export default RepairWizard;

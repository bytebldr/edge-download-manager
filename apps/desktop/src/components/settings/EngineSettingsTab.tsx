import React from "react";
import { ShieldCheck } from "lucide-react";
import type { EdmSettings } from "../../types/settings";

interface EngineSettingsTabProps {
  settings: EdmSettings;
  update: <K extends keyof EdmSettings>(section: K, patch: Partial<EdmSettings[K]>) => void;
}

export const EngineSettingsTab: React.FC<EngineSettingsTabProps> = ({
  settings,
  update,
}) => {
  return (
    <section id="section-engine" className="space-y-5">
      <div>
        <div className="flex items-center gap-2">
          <ShieldCheck className="w-4 h-4 text-[var(--accent)]" />
          <h2 className="text-sm font-semibold text-[var(--text-primary)]">Security &amp; Core Engine</h2>
        </div>
        <p className="text-xs text-[var(--text-muted)] mt-1">
          Integrity checksum verification, Windows MOTW protection, and NTFS sparse file disk pre-allocation.
        </p>
      </div>

      <div className="rounded-xl border border-[var(--border-dim)] bg-[var(--bg-card)] divide-y divide-[var(--border-dim)]">
        <div className="px-4 py-3">
          <label className="text-xs font-semibold text-[var(--text-primary)] block">File Integrity &amp; MOTW</label>
        </div>
        <label className="px-4 py-3 flex items-center justify-between gap-4 cursor-pointer group">
          <span className="text-xs font-medium text-[var(--text-secondary)] group-hover:text-[var(--text-primary)]">
            Automatically calculate and verify SHA-256 / MD5 checksums after transfer
          </span>
          <input
            type="checkbox"
            checked={settings.security.verifyChecksums}
            onChange={(e) => update("security", { verifyChecksums: e.target.checked })}
            className="edm-checkbox"
          />
        </label>

        <label className="px-4 py-3 flex items-center justify-between gap-4 cursor-pointer group">
          <span className="text-xs font-medium text-[var(--text-secondary)] group-hover:text-[var(--text-primary)]">
            Warn and block unencrypted plain HTTP downloads
          </span>
          <input
            type="checkbox"
            checked={settings.security.blockInsecureHttp}
            onChange={(e) => update("security", { blockInsecureHttp: e.target.checked })}
            className="edm-checkbox"
          />
        </label>
      </div>

      <div className="rounded-xl border border-[var(--border-dim)] bg-[var(--bg-card)] p-4 space-y-3">
        <label className="text-xs font-semibold text-[var(--text-primary)] block">
          High-Performance Engine Tuning
        </label>
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
          <div className="p-3 rounded-lg bg-[var(--bg-chassis)] border border-[var(--border-dim)]">
            <span className="text-xs font-semibold text-[var(--text-primary)] block">
              NTFS Sparse Allocation
            </span>
            <span className="text-[11px] text-[var(--text-muted)] mt-0.5 block">
              Instantly reserves cluster offsets with 0ms disk overhead and zero file fragmentation.
            </span>
            <span className="mt-2 inline-block text-[10px] font-mono text-emerald-400">
              FSCTL_SET_SPARSE Enabled
            </span>
          </div>

          <div className="p-3 rounded-lg bg-[var(--bg-chassis)] border border-[var(--border-dim)]">
            <span className="text-xs font-semibold text-[var(--text-primary)] block">
              Flush-Before-Checkpoint
            </span>
            <span className="text-[11px] text-[var(--text-muted)] mt-0.5 block">
              Guarantees clean recovery without corruption during sudden system crashes or power outages.
            </span>
            <span className="mt-2 inline-block text-[10px] font-mono text-emerald-400">
              Crash-Consistent WAL
            </span>
          </div>
        </div>
      </div>
    </section>
  );
};

import React from "react";
import { Clock } from "lucide-react";
import type { EdmSettings } from "../../types/settings";
import { AutomationPanel } from "../../features/automation";

interface AutomationSettingsTabProps {
  settings: EdmSettings;
  update: <K extends keyof EdmSettings>(section: K, patch: Partial<EdmSettings[K]>) => void;
}

export const AutomationSettingsTab: React.FC<AutomationSettingsTabProps> = ({
  settings,
  update,
}) => {
  return (
    <section id="section-automation" className="space-y-5">
      <div>
        <div className="flex items-center gap-2">
          <Clock className="w-4 h-4 text-[var(--accent)]" />
          <h2 className="text-sm font-semibold text-[var(--text-primary)]">Alerts &amp; Notifications</h2>
        </div>
        <p className="text-xs text-[var(--text-muted)] mt-1">
          Configure desktop toast alerts and audio cues upon download completion.
        </p>
      </div>

      <div className="rounded-xl border border-[var(--border-dim)] bg-[var(--bg-card)] divide-y divide-[var(--border-dim)]">
        <div className="px-4 py-3">
          <label className="text-xs font-semibold text-[var(--text-primary)] block">
            Notification &amp; Audio Alerts
          </label>
        </div>

        <label className="px-4 py-3 flex items-center justify-between gap-4 cursor-pointer group">
          <span className="text-xs font-medium text-[var(--text-secondary)] group-hover:text-[var(--text-primary)]">
            Show OS desktop banner when a download finishes
          </span>
          <input
            type="checkbox"
            checked={settings.notifications.downloadCompleted}
            onChange={(e) => update("notifications", { downloadCompleted: e.target.checked })}
            className="edm-checkbox"
          />
        </label>

        <label className="px-4 py-3 flex items-center justify-between gap-4 cursor-pointer group">
          <span className="text-xs font-medium text-[var(--text-secondary)] group-hover:text-[var(--text-primary)]">
            Play acoustic completion chime on success
          </span>
          <input
            type="checkbox"
            checked={settings.notifications.playSound}
            onChange={(e) => update("notifications", { playSound: e.target.checked })}
            className="edm-checkbox"
          />
        </label>

        <label className="px-4 py-3 flex items-center justify-between gap-4 cursor-pointer group">
          <span className="text-xs font-medium text-[var(--text-secondary)] group-hover:text-[var(--text-primary)]">
            Warn when disk free space falls below threshold (&lt; 2 GB)
          </span>
          <input
            type="checkbox"
            checked={settings.notifications.lowDiskSpace}
            onChange={(e) => update("notifications", { lowDiskSpace: e.target.checked })}
            className="edm-checkbox"
          />
        </label>
      </div>

      <AutomationPanel />
    </section>
  );
};

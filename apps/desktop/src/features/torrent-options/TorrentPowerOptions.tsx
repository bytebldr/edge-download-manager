import React, { useState } from "react";
import { ChevronDown, ChevronRight, SlidersHorizontal } from "lucide-react";

export interface TorrentPowerConfig {
  sequential: boolean;
  seed_ratio_limit: number | null;
  seed_time_limit_mins: number | null;
  create_subfolder: boolean;
  custom_folder_name: string | null;
  auto_update_trackers: boolean;
  enable_dht: boolean;
  enable_pex: boolean;
}

export const DEFAULT_TORRENT_POWER_CONFIG: TorrentPowerConfig = {
  sequential: false,
  seed_ratio_limit: null,
  seed_time_limit_mins: null,
  create_subfolder: true,
  custom_folder_name: null,
  auto_update_trackers: false,
  enable_dht: true,
  enable_pex: true,
};

interface TorrentPowerOptionsProps {
  value: TorrentPowerConfig;
  onChange: (next: TorrentPowerConfig) => void;
  defaultExpanded?: boolean;
}

export const TorrentPowerOptions: React.FC<TorrentPowerOptionsProps> = ({
  value,
  onChange,
  defaultExpanded = false,
}) => {
  const [expanded, setExpanded] = useState(defaultExpanded);

  const update = (patch: Partial<TorrentPowerConfig>) => {
    onChange({ ...value, ...patch });
  };

  return (
    <div className="rounded-xl card-3d border border-[var(--border-subtle)] text-xs overflow-hidden">
      <button
        type="button"
        onClick={() => setExpanded((prev) => !prev)}
        className="w-full px-3 py-2 flex items-center justify-between text-left hover:bg-white/5 transition-colors cursor-pointer"
      >
        <span className="flex items-center gap-2 font-medium text-[var(--text-primary)]">
          <SlidersHorizontal className="w-3.5 h-3.5 text-[var(--accent)]" />
          <span>Torrent Power Options</span>
        </span>
        {expanded ? (
          <ChevronDown className="w-3.5 h-3.5 text-[var(--text-secondary)]" />
        ) : (
          <ChevronRight className="w-3.5 h-3.5 text-[var(--text-secondary)]" />
        )}
      </button>

      {expanded && (
        <div className="px-3 pb-3 pt-2 border-t border-[var(--border-subtle)] space-y-2.5">
          <div className="space-y-1.5">
            <label className="flex items-center gap-2 cursor-pointer text-[var(--text-primary)]">
              <input
                type="checkbox"
                checked={value.create_subfolder}
                onChange={(e) => update({ create_subfolder: e.target.checked })}
                className="accent-[var(--accent)] cursor-pointer"
              />
              <span>Create subfolder</span>
            </label>

            {value.create_subfolder && (
              <div className="pl-5">
                <label className="block text-[11px] text-[var(--text-secondary)] mb-1">
                  Rename folder
                </label>
                <input
                  type="text"
                  value={value.custom_folder_name ?? ""}
                  onChange={(e) =>
                    update({
                      custom_folder_name: e.target.value.trim() ? e.target.value : null,
                    })
                  }
                  placeholder="Optional custom folder name"
                  className="w-full px-2.5 py-1.5 rounded-lg text-xs input-3d text-[var(--text-primary)] font-mono"
                />
              </div>
            )}
          </div>

          <label className="flex items-center gap-2 cursor-pointer text-[var(--text-primary)]">
            <input
              type="checkbox"
              checked={value.sequential}
              onChange={(e) => update({ sequential: e.target.checked })}
              className="accent-[var(--accent)] cursor-pointer"
            />
            <span>Download sequentially (in order)</span>
          </label>

          <label className="flex items-center gap-2 cursor-pointer text-[var(--text-primary)]">
            <input
              type="checkbox"
              checked={value.auto_update_trackers}
              onChange={(e) => update({ auto_update_trackers: e.target.checked })}
              className="accent-[var(--accent)] cursor-pointer"
            />
            <span>Auto-update trackers</span>
          </label>

          <div className="grid grid-cols-2 gap-2.5 pt-1">
            <div>
              <label className="block text-[11px] text-[var(--text-secondary)] mb-1">
                Seeding ratio cap
              </label>
              <input
                type="number"
                min={0}
                step={0.1}
                value={value.seed_ratio_limit ?? ""}
                onChange={(e) => {
                  const raw = e.target.value;
                  const parsed = raw === "" ? null : Number(raw);
                  update({
                    seed_ratio_limit:
                      parsed !== null && Number.isFinite(parsed) && parsed >= 0
                        ? parsed
                        : null,
                  });
                }}
                placeholder="No limit"
                className="w-full px-2.5 py-1.5 rounded-lg text-xs input-3d text-[var(--text-primary)] font-mono"
              />
            </div>

            <div>
              <label className="block text-[11px] text-[var(--text-secondary)] mb-1">
                Seeding time limit (mins)
              </label>
              <input
                type="number"
                min={0}
                step={1}
                value={value.seed_time_limit_mins ?? ""}
                onChange={(e) => {
                  const raw = e.target.value;
                  const parsed = raw === "" ? null : Math.floor(Number(raw));
                  update({
                    seed_time_limit_mins:
                      parsed !== null && Number.isFinite(parsed) && parsed >= 0
                        ? parsed
                        : null,
                  });
                }}
                placeholder="No limit"
                className="w-full px-2.5 py-1.5 rounded-lg text-xs input-3d text-[var(--text-primary)] font-mono"
              />
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

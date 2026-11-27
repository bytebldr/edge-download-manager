import React from "react";
import { Laptop, Moon, Sliders, Sun } from "lucide-react";
import type { EdmSettings } from "../../types/settings";
import { FONT_OPTIONS, type FontFamilyId } from "../../lib/fonts";
import type { Locale } from "../../features/i18n";
import { Select } from "../Select";
import { tauriSetAutostart } from "../../services/tauriBridge";

interface GeneralSettingsTabProps {
  settings: EdmSettings;
  update: <K extends keyof EdmSettings>(section: K, patch: Partial<EdmSettings[K]>) => void;
  handleSettingChange: (section: keyof EdmSettings, key: string, val: unknown) => void;
  paletteOptions: readonly { readonly id: string; readonly label: string; readonly color: string }[];
  locale: Locale;
  setLocale: (loc: Locale) => void;
  availableLocales: readonly { readonly code: string; readonly nativeLabel: string; readonly label: string }[];
}

export const GeneralSettingsTab: React.FC<GeneralSettingsTabProps> = ({
  settings,
  update,
  handleSettingChange,
  paletteOptions,
  locale,
  setLocale,
  availableLocales,
}) => {
  return (
    <section id="section-general" className="space-y-5">
      <div>
        <div className="flex items-center gap-2">
          <Sliders className="w-4 h-4 text-[var(--accent)]" />
          <h2 className="text-sm font-semibold text-[var(--text-primary)]">General &amp; Interface</h2>
        </div>
        <p className="text-xs text-[var(--text-muted)] mt-1">
          Configure visual appearance, theme modes, language localization, and system startup lifecycle.
        </p>
      </div>

      {/* Theme & Palette */}
      <div className="rounded-xl border border-[var(--border-dim)] bg-[var(--bg-card)] divide-y divide-[var(--border-dim)]">
        <div className="p-4 flex flex-col sm:flex-row sm:items-center justify-between gap-3">
          <div>
            <label className="text-xs font-semibold text-[var(--text-primary)] block">Theme Mode</label>
            <span className="text-[11px] text-[var(--text-muted)]">
              High-contrast dark theme or system automatic synchronization.
            </span>
          </div>
          <div className="inline-flex rounded-lg p-0.5 bg-[var(--bg-chassis)] border border-[var(--border-dim)] gap-0.5 shrink-0">
            {[
              { id: "dark" as const, label: "Dark", icon: Moon },
              { id: "light" as const, label: "Light", icon: Sun },
              { id: "system" as const, label: "System", icon: Laptop },
            ].map((m) => {
              const Icon = m.icon;
              const isSel = settings.general.theme === m.id;
              return (
                <button
                  key={m.id}
                  onClick={() => update("general", { theme: m.id })}
                  className={`py-1 px-2.5 rounded-md text-xs font-medium flex items-center gap-1.5 cursor-pointer transition-colors ${
                    isSel
                      ? "bg-[var(--accent)] text-[var(--accent-on)] font-semibold"
                      : "text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-white/[0.04]"
                  }`}
                >
                  <Icon className="w-3.5 h-3.5" />
                  <span>{m.label}</span>
                </button>
              );
            })}
          </div>
        </div>

        {/* Accent Color & Font Family (2-Column Split) */}
        <div className="p-4 grid grid-cols-1 sm:grid-cols-2 gap-4">
          <div className="flex flex-col justify-between gap-2">
            <div>
              <label className="text-xs font-semibold text-[var(--text-primary)] block">
                Accent Color
              </label>
              <span className="text-[11px] text-[var(--text-muted)]">
                Primary accent across progress, actions, and focus rings.
              </span>
            </div>
            <Select
              value={settings.general.palette}
              onChange={(val) => update("general", { palette: val as any })}
              ariaLabel="Accent Color Palette"
              className="w-full"
              options={paletteOptions.map((p) => ({
                value: p.id,
                label: p.label,
                dotColor: p.color,
              }))}
            />
          </div>

          <div className="flex flex-col justify-between gap-2">
            <div>
              <label className="text-xs font-semibold text-[var(--text-primary)] block">
                Interface Font
              </label>
              <span className="text-[11px] text-[var(--text-muted)]">
                Typography family applied across the entire app.
              </span>
            </div>
            <Select
              value={(settings.general.fontFamily || "system") as FontFamilyId}
              onChange={(val) => update("general", { fontFamily: val as FontFamilyId })}
              ariaLabel="Interface Font"
              className="w-full"
              options={FONT_OPTIONS.map((f) => ({
                value: f.id,
                label: f.label,
                hint: f.previewText,
                fontFamily: f.stack,
              }))}
            />
          </div>
        </div>

        {/* UI Style / Density */}
        <div className="p-4 flex flex-col sm:flex-row sm:items-center justify-between gap-3">
          <div>
            <label className="text-xs font-semibold text-[var(--text-primary)] block">UI Density &amp; Visual Spacing</label>
            <span className="text-[11px] text-[var(--text-muted)]">
              Configure row padding and information visual density.
            </span>
          </div>
          <div className="inline-flex rounded-lg p-0.5 bg-[var(--bg-chassis)] border border-[var(--border-dim)] gap-0.5 shrink-0">
            {[
              { id: "pro" as const, label: "Pro Density" },
              { id: "compact" as const, label: "Compact" },
              { id: "modern" as const, label: "Comfortable" },
            ].map((s) => (
              <button
                key={s.id}
                onClick={() => update("general", { uiStyle: s.id })}
                className={`px-2.5 py-1 text-xs rounded-md font-medium transition-colors cursor-pointer ${
                  settings.general.uiStyle === s.id
                    ? "bg-[var(--accent)] text-[var(--accent-on)] font-semibold"
                    : "text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-white/[0.04]"
                }`}
              >
                {s.label}
              </button>
            ))}
          </div>
        </div>

        {/* Language Selector */}
        <div className="p-4 flex flex-col sm:flex-row sm:items-center justify-between gap-3">
          <div>
            <label className="text-xs font-semibold text-[var(--text-primary)] block">Language</label>
            <span className="text-[11px] text-[var(--text-muted)]">
              Interface language and text direction (RTL supported for Arabic).
            </span>
          </div>
          <Select
            ariaLabel="Language"
            value={locale}
            onChange={(val) => setLocale(val as Locale)}
            align="right"
            className="w-48 shrink-0"
            options={availableLocales.map((loc) => ({
              value: loc.code,
              label: `${loc.nativeLabel} (${loc.label})`,
            }))}
          />
        </div>
      </div>

      {/* Startup & Lifecycle Options */}
      <div className="rounded-xl border border-[var(--border-dim)] bg-[var(--bg-card)] divide-y divide-[var(--border-dim)]">
        <div className="px-4 py-3">
          <label className="text-xs font-semibold text-[var(--text-primary)] block">
            System &amp; Background Lifecycle
          </label>
          <p className="text-[11px] text-[var(--text-muted)] mt-0.5">
            Control system tray residence, auto-start behavior, and memory-saving listener sleep.
          </p>
        </div>

        <div className="px-4 py-3 flex items-center justify-between gap-4">
          <div>
            <span className="font-medium text-xs text-[var(--text-primary)] block">Launch at Windows Startup</span>
            <span className="text-[11px] text-[var(--text-muted)]">Open EDM automatically when signing into Windows.</span>
          </div>
          <input
            type="checkbox"
            checked={settings.general.launchAtStartup}
            onChange={(e) => {
              const val = e.target.checked;
              update("general", { launchAtStartup: val });
              tauriSetAutostart(val);
            }}
            className="edm-checkbox"
          />
        </div>

        {settings.general.launchAtStartup && (
          <div className="px-4 py-3 pl-8 flex items-center justify-between gap-4 bg-white/[0.01]">
            <div>
              <span className="font-medium text-xs text-[var(--text-primary)] block">Start Minimized</span>
              <span className="text-[11px] text-[var(--text-muted)]">Launch directly into the notification tray without opening the window.</span>
            </div>
            <input
              type="checkbox"
              checked={settings.general.startMinimized}
              onChange={(e) => update("general", { startMinimized: e.target.checked })}
              className="edm-checkbox"
            />
          </div>
        )}

        <div className="px-4 py-3 flex items-center justify-between gap-4">
          <div>
            <span className="font-medium text-xs text-[var(--text-primary)] block">Minimize to System Tray</span>
            <span className="text-[11px] text-[var(--text-muted)]">Closing the window keeps downloads active in the background.</span>
          </div>
          <input
            type="checkbox"
            checked={settings.general.minimizeToTray}
            onChange={(e) => update("general", { minimizeToTray: e.target.checked })}
            className="edm-checkbox"
          />
        </div>

        <div className="px-4 py-3 flex items-center justify-between gap-4">
          <div>
            <span className="font-medium text-xs text-[var(--text-primary)] block">Auto-Restore Unfinished Downloads</span>
            <span className="text-[11px] text-[var(--text-muted)]">Automatically resume paused or incomplete tasks upon launch.</span>
          </div>
          <input
            type="checkbox"
            checked={settings.general.restoreDownloadsOnLaunch}
            onChange={(e) => update("general", { restoreDownloadsOnLaunch: e.target.checked })}
            className="edm-checkbox"
          />
        </div>

        <div className="px-4 py-3 flex items-center justify-between gap-4">
          <div>
            <span className="font-medium text-xs text-[var(--text-primary)] block">Confirm Before Exit</span>
            <span className="text-[11px] text-[var(--text-muted)]">Prompt for confirmation if active downloads are transferring.</span>
          </div>
          <input
            type="checkbox"
            checked={settings.general.confirmBeforeExit}
            onChange={(e) => update("general", { confirmBeforeExit: e.target.checked })}
            className="edm-checkbox"
          />
        </div>

        <div className="px-4 py-3 flex items-center justify-between gap-4">
          <div>
            <span className="font-medium text-xs text-[var(--text-primary)] block">Keep Engine Awake</span>
            <span className="text-[11px] text-[var(--text-muted)]">Prevent EDM from dropping into listener sleep and releasing WebView2 RAM.</span>
          </div>
          <input
            type="checkbox"
            checked={settings.general.keepAwake ?? false}
            onChange={(e) => update("general", { keepAwake: e.target.checked })}
            className="edm-checkbox"
          />
        </div>

        <div className="px-4 py-3 flex flex-col sm:flex-row sm:items-center justify-between gap-3">
          <div>
            <span className="font-medium text-xs text-[var(--text-primary)] block">Idle Exit Timeout</span>
            <span className="text-[11px] text-[var(--text-muted)]">Completely exit background processes after a period of inactivity.</span>
          </div>
          <Select
            value={
              settings.general.idleExitMinutes === null || settings.general.idleExitMinutes === undefined
                ? "never"
                : String(settings.general.idleExitMinutes)
            }
            disabled={settings.general.keepAwake}
            onChange={(v) => {
              const val = v === "never" ? null : parseInt(v, 10);
              update("general", { idleExitMinutes: val });
            }}
            ariaLabel="Idle Exit Timeout"
            align="right"
            className="w-60 shrink-0"
            options={[
              { value: "never", label: "Never (Stay in Listener Sleep)" },
              { value: "0", label: "Immediately after Listener Sleep" },
              { value: "5", label: "After 5 minutes idle" },
              { value: "15", label: "After 15 minutes idle (Default)" },
              { value: "30", label: "After 30 minutes idle" },
              { value: "60", label: "After 60 minutes idle" },
            ]}
          />
        </div>

        <div className="px-4 py-3 flex items-center justify-between gap-4">
          <div>
            <span className="font-medium text-xs text-[var(--text-primary)] block">Show Connection Stream Boundaries</span>
            <span className="text-[11px] text-[var(--text-muted)]">Render subtle boundary indicators for parallel workers on progress tracks.</span>
          </div>
          <input
            type="checkbox"
            checked={settings.general.showConnectionBoundaries ?? false}
            onChange={(e) => update("general", { showConnectionBoundaries: e.target.checked })}
            className="edm-checkbox"
          />
        </div>

        <div className="px-4 py-3 flex flex-col sm:flex-row sm:items-center justify-between gap-3">
          <div>
            <span className="font-medium text-xs text-[var(--text-primary)] block">Download Deletion Policy</span>
            <span className="text-[11px] text-[var(--text-muted)]">Choose default behavior when removing downloads from your list.</span>
          </div>
          <Select
            value={settings.general.deletePreference || "always_ask"}
            onChange={(val) => handleSettingChange("general", "deletePreference", val)}
            ariaLabel="Download Deletion Policy"
            align="right"
            className="w-64 shrink-0"
            options={[
              { value: "always_ask", label: "Always ask (show confirmation)" },
              { value: "list_only", label: "Remove from list only (keep files)" },
              { value: "delete_file", label: "Delete file and remove from list" },
            ]}
          />
        </div>
      </div>

      {/* Global Shortcuts Matrix (General & Interface) */}
      <div className="rounded-xl border border-[var(--border-dim)] bg-[var(--bg-card)] p-4 space-y-3">
        <label className="text-xs font-semibold text-[var(--text-primary)] block">
          Keyboard Shortcuts Reference
        </label>
        <div className="grid grid-cols-2 sm:grid-cols-3 gap-2 text-xs">
          {[
            { key: "Ctrl + N", desc: "Add New URL" },
            { key: "Space", desc: "Pause / Resume" },
            { key: "Delete", desc: "Remove Task" },
            { key: "Ctrl + ,", desc: "Preferences" },
            { key: "Enter", desc: "Download Status" },
            { key: "Esc", desc: "Close / Back" },
          ].map((s) => (
            <div
              key={s.key}
              className="p-2 rounded-lg bg-[var(--bg-chassis)] border border-[var(--border-dim)] flex items-center justify-between font-mono"
            >
              <span className="text-[var(--text-muted)] text-[11px] font-sans">{s.desc}</span>
              <kbd className="px-1.5 py-0.5 rounded bg-[var(--bg-card)] border border-[var(--border-dim)] text-[10px] text-[var(--accent-badge-text)] font-semibold">
                {s.key}
              </kbd>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
};

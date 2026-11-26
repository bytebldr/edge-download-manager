import { useState, useRef, useEffect } from "react";
import {
  Moon,
  Sun,
  Laptop,
  Sliders,
  Download,
  Upload,
  Folder,
  FolderOpen,
  Globe,
  Clock,
  ShieldCheck,
  Cpu,
  Info,
  RotateCcw,
  Search,
  Plus,
  ExternalLink,
  HardDrive,
  Trash2,
  AlertTriangle,
  RefreshCw,
  CheckCircle2,
  X,
  Package,
  Layers,
  Sparkles,
  Share2,
  Film,
  Server,
  Radio,
  Copy,
  Check,
  Key,
  Wrench,
} from "lucide-react";
import {
  tauriGetExtensionToken,
  tauriGetAppVersion,
  tauriCheckForUpdates,
  tauriInstallUpdate,
  tauriOpenInstallerFolder,
  tauriListenOtaProgress,
  tauriSelectDownloadDirectory,
  openFolderDialog,
  tauriGetSystemUserDirs,
  tauriGetDiskFreeSpace,
  tauriSetAutostart,
  DiskFreeSpace,
  UpdateCheckResult,
  OtaProgress,
} from "../services/tauriBridge";
import { EdmSettings, defaultSettings, saveSettings } from "../types/settings";
import { FEATURE_PACKS } from "../types/featurePacks";
import { formatBytes } from "../utils/format";
import { useI18n, type Locale } from "../features/i18n";
import { RoutingAndCategoryPanel } from "../features/routing";
import { AutomationPanel } from "../features/automation";
import { motion } from "motion/react";
import { springs } from "../utils/motion";
import type { DownloadItem } from "../App";

interface SettingsPageProps {
  settings: EdmSettings;
  onUpdateSettings: (newSettings: EdmSettings) => void;
  onBack: () => void;
  speedLimit?: string;
  onSpeedLimitChange?: (val: string) => void;
  selectedDownload?: DownloadItem | null;
  totalSpeed?: string;
  totalUploadSpeed?: string;
  activeThreads?: number;
  totalSessionBytes?: number;
}

type SettingsSection =
  | "general"
  | "downloads"
  | "packs"
  | "network"
  | "automation"
  | "engine"
  | "about";

const paletteOptions = [
  { id: "blue", label: "Sapphire Blue", color: "#2563EB" },
  { id: "emerald", label: "Nordic Emerald", color: "#10B981" },
  { id: "amber", label: "Cyberpunk Amber", color: "#F59E0B" },
  { id: "violet", label: "Velvet Violet", color: "#8B5CF6" },
  { id: "rose", label: "Crimson Flame", color: "#F43F5E" },
  { id: "titanium", label: "Titanium Slate", color: "#64748B" },
] as const;

export default function SettingsPage({
  settings,
  onUpdateSettings,
}: SettingsPageProps) {
  const { locale, setLocale, availableLocales } = useI18n();
  const [activeSection, setActiveSection] = useState<SettingsSection>("general");
  const [searchQuery, setSearchQuery] = useState("");
  const [customCatName, setCustomCatName] = useState("");
  const [customCatExt, setCustomCatExt] = useState("");
  const [importStatus, setImportStatus] = useState<{ type: "success" | "error"; message: string } | null>(null);
  const [appVersion, setAppVersion] = useState<string>("0.1.1");
  const [updateStatus, setUpdateStatus] = useState<
    "idle" | "checking" | "up-to-date" | "update-available" | "downloading" | "ready" | "error"
  >("idle");
  const [updateInfo, setUpdateInfo] = useState<UpdateCheckResult | null>(null);
  const [otaProgress, setOtaProgress] = useState<OtaProgress | null>(null);
  const [updateError, setUpdateError] = useState<string | null>(null);
  const [lastCheckedTime, setLastCheckedTime] = useState<string>("Not checked yet");
  const [wsToken, setWsToken] = useState<string>("");
  const [copiedToken, setCopiedToken] = useState<boolean>(false);
  const [diskSpace, setDiskSpace] = useState<DiskFreeSpace | null>(null);
  const mainRef = useRef<HTMLElement>(null);
  const systemDirsRef = useRef<{ downloads: string; documents: string; music: string; pictures: string; videos: string; temp: string } | null>(null);

  useEffect(() => {
    tauriGetDiskFreeSpace(settings.downloads.defaultDirectory).then((res) => {
      if (res) setDiskSpace(res);
    });
  }, [settings.downloads.defaultDirectory]);

  const handleBrowseFolder = async () => {
    const chosen = await tauriSelectDownloadDirectory(settings.downloads.defaultDirectory);
    if (chosen) {
      update("downloads", { defaultDirectory: chosen });
    }
  };

  const handleBrowseCategoryFolder = async (catId: string, currentPath: string) => {
    const chosen = await openFolderDialog(currentPath || settings.downloads.defaultDirectory);
    if (chosen) {
      const updatedCats = settings.categories.map((c) =>
        c.id === catId ? { ...c, destinationDirectory: chosen } : c
      );
      onUpdateSettings({ ...settings, categories: updatedCats });
      saveSettings({ ...settings, categories: updatedCats });
    }
  };

  // Helper to ensure a Windows known folder path has exactly one trailing \EDM subfolder
  const ensureSingleEdm = (base: string): string => {
    if (!base) return "";
    const normalized = base.replace(/\//g, "\\");
    const stripped = normalized.replace(/(\\EDM)+$/i, "");
    return `${stripped}\\EDM`;
  };

  // Legacy path segments that should be migrated to Windows Known Folders
  const LEGACY_PATH_MIGRATIONS: Record<string, (dirs: NonNullable<typeof systemDirsRef.current>) => string> = {
    "Downloads\\Video":    (d) => ensureSingleEdm(d.videos),
    "Downloads\\Audio":   (d) => ensureSingleEdm(d.music),
    "Downloads\\Music":   (d) => ensureSingleEdm(d.music),
    "Downloads\\Pictures":(d) => ensureSingleEdm(d.pictures),
    "Downloads\\Documents":(d) => ensureSingleEdm(d.documents),
  };

  const AUTO_CATEGORY_PATHS = (dirs: NonNullable<typeof systemDirsRef.current>): Record<string, string> => ({
    audio:     ensureSingleEdm(dirs.music),
    music:     ensureSingleEdm(dirs.music),
    pictures:  ensureSingleEdm(dirs.pictures),
    video:     ensureSingleEdm(dirs.videos),
    programs:  `${dirs.downloads}\\Programs`,
    archives:  `${dirs.downloads}\\Archives`,
    documents: ensureSingleEdm(dirs.documents),
  });

  const handleFolderModeSwitch = async (mode: "auto" | "fixed") => {
    const dirs = systemDirsRef.current ?? await tauriGetSystemUserDirs();
    if (dirs) systemDirsRef.current = dirs;

    let updatedCats = settings.categories;
    if (mode === "auto" && dirs) {
      const autoMap = AUTO_CATEGORY_PATHS(dirs);
      updatedCats = settings.categories.map((cat) => ({
        ...cat,
        destinationDirectory: autoMap[cat.id.toLowerCase()] ?? cat.destinationDirectory,
      }));
    } else if (mode === "fixed") {
      const base = settings.downloads.defaultDirectory;
      updatedCats = settings.categories.map((cat) => ({
        ...cat,
        destinationDirectory: `${base}\\${cat.name}`,
      }));
    }

    const updated = {
      ...settings,
      storage: { ...settings.storage, folderMode: mode },
      categories: updatedCats,
    };
    onUpdateSettings(updated);
    saveSettings(updated);
  };

  useEffect(() => {
    tauriGetSystemUserDirs().then((dirs) => {
      if (!dirs) return;
      systemDirsRef.current = dirs;

      let needsSave = false;
      const updatedDownloads = { ...settings.downloads };
      if (!updatedDownloads.defaultDirectory) {
        updatedDownloads.defaultDirectory = dirs.downloads;
        needsSave = true;
      }
      if (!updatedDownloads.temporaryDirectory) {
        updatedDownloads.temporaryDirectory = dirs.temp || `${dirs.downloads}\\temp`;
        needsSave = true;
      }

      const categoryFolderMap = AUTO_CATEGORY_PATHS(dirs);

      const updatedCategories = settings.categories.map((cat) => {
        let dest = cat.destinationDirectory || "";
        const catId = cat.id.toLowerCase();

        // 1. Fix duplicate \EDM\EDM occurrences in destination
        if (dest.includes("\\EDM\\EDM") || dest.includes("/EDM/EDM")) {
          dest = dest.replace(/([\\/]EDM)+$/gi, "\\EDM");
          needsSave = true;
        }

        // 2. Audio category in Downloads\Audio -> migrate to Music\EDM
        if ((catId === "audio" || catId === "music") && (dest.includes("Downloads\\Audio") || dest.includes("Downloads/Audio") || dest.includes("Downloads\\Music") || dest.includes("Downloads/Music"))) {
          dest = ensureSingleEdm(dirs.music);
          needsSave = true;
        }

        // 3. In intelligent categorized mode ("auto"), ensure paths stay strictly mapped to standard folders
        if (settings.storage.folderMode === "auto" && categoryFolderMap[catId]) {
          if (dest !== categoryFolderMap[catId]) {
            dest = categoryFolderMap[catId];
            needsSave = true;
          }
        } else if (!dest) {
          dest = categoryFolderMap[catId] || `${dirs.downloads}\\${cat.name}`;
          needsSave = true;
        } else {
          // Check for legacy migration
          const legacyKey = Object.keys(LEGACY_PATH_MIGRATIONS).find((k) => dest.includes(k));
          if (legacyKey) {
            dest = LEGACY_PATH_MIGRATIONS[legacyKey](dirs);
            needsSave = true;
          }
        }

        return { ...cat, destinationDirectory: dest };
      });

      if (needsSave) {
        const nextSettings = {
          ...settings,
          downloads: updatedDownloads,
          categories: updatedCategories,
        };
        onUpdateSettings(nextSettings);
        saveSettings(nextSettings);
      }
    });

    tauriGetAppVersion().then((v) => {
      if (v) setAppVersion(v);
    });
    tauriGetExtensionToken().then((t) => {
      if (t) setWsToken(t);
    });

    let unlistenFn: (() => void) | undefined;
    tauriListenOtaProgress((prog) => {
      setOtaProgress(prog);
      if (prog.status === "downloading") {
        setUpdateStatus("downloading");
      } else if (prog.status === "ready") {
        setUpdateStatus("ready");
      }
    }).then((unlisten) => {
      unlistenFn = unlisten;
    });

    return () => {
      if (unlistenFn) unlistenFn();
    };
  }, []);

  const handleCopyToken = () => {
    if (wsToken) {
      navigator.clipboard.writeText(wsToken);
      setCopiedToken(true);
      setTimeout(() => setCopiedToken(false), 2000);
    }
  };

  const navItems = [
    {
      id: "general" as const,
      label: "General",
      icon: Sliders,
      keywords: "theme dark light system accent palette density language startup tray restore exit awake timeout stream boundaries delete",
    },
    {
      id: "downloads" as const,
      label: "Downloads",
      icon: Download,
      keywords: "storage folder routing directory concurrency streams threads adaptive ceiling auto byte-range collision conflict extensions category rules",
    },
    {
      id: "packs" as const,
      label: "Feature Packs",
      icon: Package,
      keywords: "http bittorrent ftp ed2k m3u8 ffmpeg yt-dlp github huggingface protocols adapters",
    },
    {
      id: "network" as const,
      label: "Network",
      icon: Globe,
      keywords: "browser extension sniffer youtube clipboard websocket token proxy socks5 http routing",
    },
    {
      id: "automation" as const,
      label: "Alerts",
      icon: Clock,
      keywords: "notifications toast sound chime low disk space automation scheduler",
    },
    {
      id: "engine" as const,
      label: "Security & Engine",
      icon: ShieldCheck,
      keywords: "checksum sha256 md5 motw http ntfs sparse allocation wal shortcuts keyboard",
    },
    {
      id: "about" as const,
      label: "About",
      icon: Info,
      keywords: "version updates ota repair backup restore export import json factory reset",
    },
  ];

  const trimmedQuery = searchQuery.trim().toLowerCase();
  const isSearching = trimmedQuery.length > 0;

  const doesNavItemMatch = (item: (typeof navItems)[number]) => {
    if (!isSearching) return true;
    return (
      item.label.toLowerCase().includes(trimmedQuery) ||
      item.keywords.toLowerCase().includes(trimmedQuery) ||
      item.id.toLowerCase().includes(trimmedQuery)
    );
  };

  const isSectionVisible = (id: SettingsSection) => {
    if (!isSearching) return activeSection === id;
    const navItem = navItems.find((item) => item.id === id);
    return navItem ? doesNavItemMatch(navItem) : false;
  };

  const handleSelectSection = (sectionId: SettingsSection) => {
    setActiveSection(sectionId);
    if (isSearching) {
      setSearchQuery("");
    }
    mainRef.current?.scrollTo({ top: 0 });
  };

  const update = <K extends keyof EdmSettings>(section: K, patch: Partial<EdmSettings[K]>) => {
    const updated = {
      ...settings,
      [section]: {
        ...(settings[section] as any),
        ...patch,
      },
    };
    onUpdateSettings(updated);
    saveSettings(updated);
  };

  const handleSettingChange = (section: any, key: string, value: any) => {
    update(section, { [key]: value });
  };

  const resetCurrentSection = () => {
    if (confirm(`Reset ${activeSection.toUpperCase()} settings to factory defaults?`)) {
      let updated = { ...settings };
      if (activeSection === "general") {
        updated.general = defaultSettings.general;
      } else if (activeSection === "downloads") {
        updated.downloads = defaultSettings.downloads;
        updated.storage = defaultSettings.storage;
        updated.categories = defaultSettings.categories;
      } else if (activeSection === "packs") {
        updated.featurePacks = defaultSettings.featurePacks;
      } else if (activeSection === "network") {
        updated.network = defaultSettings.network;
        updated.browser = defaultSettings.browser;
      } else if (activeSection === "automation") {
        updated.queues = defaultSettings.queues;
        updated.notifications = defaultSettings.notifications;
      } else if (activeSection === "engine") {
        updated.security = defaultSettings.security;
        updated.advanced = defaultSettings.advanced;
      } else if (activeSection === "about") {
        updated = defaultSettings;
      }
      onUpdateSettings(updated);
      saveSettings(updated);
    }
  };

  const handleExport = () => {
    const dataStr = "data:text/json;charset=utf-8," + encodeURIComponent(JSON.stringify(settings, null, 2));
    const downloadAnchor = document.createElement("a");
    downloadAnchor.setAttribute("href", dataStr);
    downloadAnchor.setAttribute("download", `edm-settings-v${settings.schemaVersion}.json`);
    document.body.appendChild(downloadAnchor);
    downloadAnchor.click();
    downloadAnchor.remove();
  };

  const handleImport = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = (event) => {
      try {
        const parsed = JSON.parse(event.target?.result as string);
        if (parsed.schemaVersion) {
          onUpdateSettings(parsed);
          saveSettings(parsed);
          setImportStatus({ type: "success", message: "Settings imported successfully." });
        } else {
          setImportStatus({ type: "error", message: "Invalid EDM settings file format." });
        }
      } catch (err) {
        setImportStatus({ type: "error", message: "Failed to parse settings JSON file." });
      }
      setTimeout(() => setImportStatus(null), 4000);
    };
    reader.readAsText(file);
  };

  const handleCheckUpdate = async () => {
    setUpdateStatus("checking");
    setUpdateError(null);
    try {
      const res = await tauriCheckForUpdates();
      setUpdateInfo(res);
      const now = new Date();
      setLastCheckedTime(
        `Today, ${now.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}`
      );
      if (res.update_available) {
        setUpdateStatus("update-available");
      } else {
        setUpdateStatus("up-to-date");
      }
    } catch (err: any) {
      setUpdateError(err?.message || String(err));
      setUpdateStatus("error");
    }
  };

  const handleInstallUpdate = async () => {
    if (!updateInfo?.download_url) return;
    setUpdateStatus("downloading");
    setOtaProgress({
      downloaded_bytes: 0,
      total_bytes: updateInfo.asset_size || 0,
      percent: 0,
      status: "downloading",
    });
    try {
      await tauriInstallUpdate(updateInfo.download_url);
    } catch (err: any) {
      setUpdateError(err?.message || String(err));
      setUpdateStatus("error");
    }
  };

  const handleAddCategory = () => {
    if (!customCatName.trim() || !customCatExt.trim()) return;
    const exts = customCatExt
      .split(",")
      .map((e) => e.trim().toLowerCase().replace(/^\./, ""))
      .filter(Boolean);
    if (!exts.length) return;

    const newCat = {
      id: customCatName.toLowerCase().replace(/[^a-z0-9]/g, "-"),
      name: customCatName.trim(),
      enabled: true,
      extensions: exts,
      destinationDirectory: `${settings.downloads.defaultDirectory}\\${customCatName.trim()}`,
      priority: "normal" as const,
    };

    const updated = {
      ...settings,
      categories: [...settings.categories, newCat],
    };
    onUpdateSettings(updated);
    saveSettings(updated);
    setCustomCatName("");
    setCustomCatExt("");
  };

  const handleDeleteCategory = (catId: string) => {
    const updated = {
      ...settings,
      categories: settings.categories.filter((c) => c.id !== catId),
    };
    onUpdateSettings(updated);
    saveSettings(updated);
  };

  const filteredNavItems = navItems.filter(doesNavItemMatch);

  return (
    <div className="flex flex-col h-full w-full min-h-0 overflow-hidden text-sm bg-[var(--bg-chassis)] text-[var(--text-primary)]">
      {/* ─────────────────────────────────────────────────────────────
          1. INLINE UTILITY STRIP
      ────────────────────────────────────────────────────────────── */}
      <div className="h-11 px-4 sm:px-6 border-b border-[var(--border-dim)] flex items-center justify-between gap-3 shrink-0 select-none bg-[var(--bg-chassis)]">
        <div className="flex items-center gap-2 shrink-0">
          <span className="font-semibold text-xs sm:text-sm text-[var(--text-primary)] tracking-tight whitespace-nowrap">
            Preferences
          </span>
          <span className="text-[10px] px-1.5 py-0.5 rounded font-mono bg-[var(--bg-card)] border border-[var(--border-dim)] text-[var(--text-muted)] hidden md:inline">
            v{appVersion}
          </span>
        </div>

        {/* Search & Global Actions */}
        <div className="flex items-center gap-2 shrink-0">
          <div className="relative">
            <Search className="w-3.5 h-3.5 absolute left-2.5 top-1/2 -translate-y-1/2 text-[var(--text-muted)] pointer-events-none" />
            <input
              type="text"
              placeholder="Search settings..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="w-48 sm:w-64 h-8 pl-8 pr-7 rounded-lg text-xs bg-[var(--bg-card)] border border-[var(--border-subtle)] text-[var(--text-primary)] placeholder-[var(--text-muted)] focus:outline-none focus:border-[var(--accent)] transition-colors truncate"
            />
            {searchQuery && (
              <button
                onClick={() => setSearchQuery("")}
                aria-label="Clear search"
                title="Clear search"
                className="absolute right-2 top-1/2 -translate-y-1/2 text-[var(--text-muted)] hover:text-[var(--text-primary)] cursor-pointer"
              >
                <X className="w-3.5 h-3.5" />
              </button>
            )}
          </div>

          <button
            onClick={resetCurrentSection}
            className="h-8 px-2.5 rounded-lg border border-[var(--border-subtle)] bg-transparent hover:bg-white/[0.05] text-xs font-medium text-[var(--text-secondary)] hover:text-amber-400 inline-flex items-center gap-1.5 cursor-pointer whitespace-nowrap shrink-0 transition-colors"
            title="Reset active category to factory defaults"
          >
            <RotateCcw className="w-3.5 h-3.5" />
            <span className="hidden lg:inline">Reset</span>
          </button>

          <button
            onClick={handleExport}
            className="h-8 px-2.5 rounded-lg border border-[var(--border-subtle)] bg-transparent hover:bg-white/[0.05] text-xs font-medium text-[var(--text-secondary)] hover:text-[var(--text-primary)] inline-flex items-center gap-1.5 cursor-pointer whitespace-nowrap shrink-0 transition-colors"
            title="Export settings JSON"
          >
            <Download className="w-3.5 h-3.5" />
            <span className="hidden sm:inline">Export</span>
          </button>
        </div>
      </div>

      {/* ─────────────────────────────────────────────────────────────
          2. SPLIT LAYOUT: COMPACT RAIL + MAIN CONFIG CANVAS
      ────────────────────────────────────────────────────────────── */}
      <div className="flex flex-1 min-h-0 overflow-hidden">
        {/* Navigation Rail */}
        <aside className="w-48 md:w-52 border-r border-[var(--border-dim)] bg-[var(--bg-sidebar)] p-2 space-y-0.5 flex flex-col shrink-0 overflow-y-auto">
          {filteredNavItems.map((item) => {
            const Icon = item.icon;
            const isActive = !isSearching && activeSection === item.id;
            return (
              <button
                key={item.id}
                onClick={() => handleSelectSection(item.id)}
                className={`w-full h-8 flex items-center gap-2 rounded-lg px-2.5 py-1.5 text-xs font-medium text-left transition-colors cursor-pointer relative ${
                  isActive
                    ? "bg-[var(--accent-badge-bg)] text-[var(--accent-badge-text)]"
                    : "text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-white/[0.04]"
                }`}
              >
                {isActive && (
                  <span className="absolute left-0 top-1.5 bottom-1.5 w-[2px] rounded-r bg-[var(--accent)]" />
                )}
                <Icon className={`w-3.5 h-3.5 shrink-0 ${isActive ? "text-[var(--accent-badge-text)]" : "text-[var(--text-muted)]"}`} />
                <span className="truncate">{item.label}</span>
              </button>
            );
          })}
          {filteredNavItems.length === 0 && (
            <div className="px-2.5 py-3 text-[11px] text-[var(--text-muted)]">
              No matching categories
            </div>
          )}
        </aside>

        {/* Main Configuration Viewport */}
        <main
          ref={mainRef}
          className="flex-1 min-w-0 bg-[var(--bg-chassis)] overflow-y-auto px-5 py-5 sm:px-8 custom-scrollbar"
        >
          <div className="max-w-4xl w-full mx-auto space-y-8">
          {isSearching && filteredNavItems.length === 0 && (
            <div className="rounded-xl border border-[var(--border-dim)] bg-[var(--bg-card)] p-8 text-center">
              <p className="text-xs font-medium text-[var(--text-secondary)]">
                No settings match &ldquo;{searchQuery}&rdquo;
              </p>
              <button
                onClick={() => setSearchQuery("")}
                className="mt-3 h-7 px-3 rounded-lg border border-[var(--border-subtle)] text-xs text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-white/[0.05] cursor-pointer transition-colors"
              >
                Clear search
              </button>
            </div>
          )}
          {/* ═════════════════════════════════════════════════════════
              SECTION 1: GENERAL & INTERFACE
          ══════════════════════════════════════════════════════════ */}
          {isSectionVisible("general") && (
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

                {/* Accent Color Palettes */}
                <div className="p-4 space-y-2.5">
                  <div>
                    <label className="text-xs font-semibold text-[var(--text-primary)] block">
                      Accent Color Palette
                    </label>
                    <span className="text-[11px] text-[var(--text-muted)]">
                      Select your preferred accent tone across progress indicators, buttons, and telemetry.
                    </span>
                  </div>
                  <div className="flex flex-wrap gap-2 pt-0.5">
                    {paletteOptions.map((p) => {
                      const isSel = settings.general.palette === p.id;
                      return (
                        <button
                          key={p.id}
                          onClick={() => update("general", { palette: p.id as any })}
                          className={`inline-flex items-center gap-2 px-2.5 py-1.5 rounded-lg border text-xs cursor-pointer transition-colors ${
                            isSel
                              ? "border-[var(--accent)] bg-[var(--accent-badge-bg)] text-[var(--text-primary)] font-semibold"
                              : "border-[var(--border-dim)] bg-transparent text-[var(--text-secondary)] hover:border-[var(--border-subtle)] hover:text-[var(--text-primary)]"
                          }`}
                        >
                          <span
                            className="w-3.5 h-3.5 rounded-full shrink-0"
                            style={{ backgroundColor: p.color }}
                          />
                          <span className="text-[11px] whitespace-nowrap">{p.label}</span>
                        </button>
                      );
                    })}
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
                  <select
                    aria-label="Language"
                    value={locale}
                    onChange={(e) => setLocale(e.target.value as Locale)}
                    className="h-8 px-3 rounded-lg bg-[var(--bg-chassis)] border border-[var(--border-subtle)] text-xs font-medium text-[var(--text-primary)] cursor-pointer shrink-0"
                  >
                    {availableLocales.map((loc) => (
                      <option key={loc.code} value={loc.code}>
                        {loc.nativeLabel} ({loc.label})
                      </option>
                    ))}
                  </select>
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
                  <select
                    value={
                      settings.general.idleExitMinutes === null || settings.general.idleExitMinutes === undefined
                        ? "never"
                        : String(settings.general.idleExitMinutes)
                    }
                    disabled={settings.general.keepAwake}
                    onChange={(e) => {
                      const val = e.target.value === "never" ? null : parseInt(e.target.value, 10);
                      update("general", { idleExitMinutes: val });
                    }}
                    className="h-8 px-2.5 rounded-lg bg-[var(--bg-chassis)] border border-[var(--border-subtle)] text-xs text-[var(--text-primary)] focus:outline-none focus:border-[var(--accent)] cursor-pointer disabled:opacity-40 shrink-0"
                  >
                    <option value="never">Never (Stay in Listener Sleep)</option>
                    <option value="0">Immediately after Listener Sleep</option>
                    <option value="5">After 5 minutes idle</option>
                    <option value="15">After 15 minutes idle (Default)</option>
                    <option value="30">After 30 minutes idle</option>
                    <option value="60">After 60 minutes idle</option>
                  </select>
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
                  <select
                    value={settings.general.deletePreference || "always_ask"}
                    onChange={(e) => handleSettingChange("general", "deletePreference", e.target.value)}
                    className="h-8 px-2.5 rounded-lg bg-[var(--bg-chassis)] border border-[var(--border-subtle)] text-xs text-[var(--text-primary)] focus:outline-none focus:border-[var(--accent)] cursor-pointer shrink-0"
                  >
                    <option value="always_ask">Always ask (show confirmation modal)</option>
                    <option value="list_only">Remove from list only (always keep files on disk)</option>
                    <option value="delete_file">Delete file and remove from list (Recycle Bin / fallback)</option>
                  </select>
                </div>
              </div>
            </section>
          )}

          {/* ═════════════════════════════════════════════════════════
              SECTION 2: DOWNLOADS & STORAGE (Smart Storage & Routing)
          ══════════════════════════════════════════════════════════ */}
          {isSectionVisible("downloads") && (
            <section id="section-downloads" className="space-y-5">
              <div>
                <div className="flex items-center gap-2">
                  <Download className="w-4 h-4 text-[var(--accent)]" />
                  <h2 className="text-sm font-semibold text-[var(--text-primary)]">Downloads &amp; Storage</h2>
                </div>
                <p className="text-xs text-[var(--text-muted)] mt-1">
                  Manage intelligent folder routing, multi-stream concurrency limits, conflict handling, and category maps.
                </p>
              </div>

              {/* Default Download Folder Strategy */}
              <div className="rounded-xl border border-[var(--border-dim)] bg-[var(--bg-card)] divide-y divide-[var(--border-dim)]">
                <div className="p-4 flex items-center justify-between gap-3">
                  <div>
                    <label className="text-xs font-semibold text-[var(--text-primary)] block">
                      Download Directory &amp; Storage Strategy
                    </label>
                    <span className="text-[11px] text-[var(--text-muted)]">
                      Choose between category-based automated sorting or a fixed custom directory.
                    </span>
                  </div>
                  <span className="text-[10px] uppercase font-mono px-2 py-0.5 rounded bg-[var(--accent-badge-bg)] text-[var(--accent-badge-text)] font-semibold border border-[var(--accent-badge-border)] shrink-0">
                    Smart Engine
                  </span>
                </div>

                <div className="p-4 grid grid-cols-1 md:grid-cols-2 gap-3">
                  {/* Option 1: Automatic Folder Routing */}
                  <div
                    onClick={() => handleFolderModeSwitch("auto")}
                    className={`p-3.5 rounded-lg border transition-colors cursor-pointer flex flex-col justify-between ${
                      settings.storage.folderMode === "auto"
                        ? "bg-[var(--bg-active)] border-[var(--accent)]"
                        : "bg-[var(--bg-chassis)] border-[var(--border-dim)] hover:border-[var(--border-subtle)]"
                    }`}
                  >
                    <div>
                      <div className="flex items-center justify-between gap-2">
                        <div className="flex items-center gap-2">
                          <div
                            className={`w-4 h-4 rounded-full border flex items-center justify-center shrink-0 ${
                              settings.storage.folderMode === "auto"
                                ? "border-[var(--accent)] bg-[var(--accent)]"
                                : "border-[var(--border-subtle)] bg-[var(--bg-card)]"
                            }`}
                          >
                            {settings.storage.folderMode === "auto" && <div className="w-1.5 h-1.5 rounded-full bg-white" />}
                          </div>
                          <span className="font-semibold text-xs text-[var(--text-primary)]">
                            Intelligent Categorization
                          </span>
                        </div>
                        <span className="px-1.5 py-0.5 rounded text-[10px] font-semibold bg-emerald-500/15 text-emerald-400 border border-emerald-500/30">
                          Recommended
                        </span>
                      </div>
                      <p className="text-[11px] text-[var(--text-muted)] mt-2">
                        Inspects MIME signatures to auto-sort into Programs, Videos, Music, Documents, and Archives.
                      </p>
                    </div>

                    <div className="mt-3 pt-3 border-t border-[var(--border-dim)] space-y-2">
                      <label
                        className="flex items-center gap-2.5 cursor-pointer"
                        onClick={(e) => e.stopPropagation()}
                      >
                        <input
                          type="checkbox"
                          checked={settings.storage.suggestFoldersByType}
                          onChange={(e) => update("storage", { suggestFoldersByType: e.target.checked })}
                          className="edm-checkbox"
                        />
                        <span className="text-[11px] text-[var(--text-secondary)] font-medium">
                          Sort into category subfolders
                        </span>
                      </label>
                      <label
                        className="flex items-center gap-2.5 cursor-pointer"
                        onClick={(e) => e.stopPropagation()}
                      >
                        <input
                          type="checkbox"
                          checked={settings.storage.suggestFoldersByDomain}
                          onChange={(e) => update("storage", { suggestFoldersByDomain: e.target.checked })}
                          className="edm-checkbox"
                        />
                        <span className="text-[11px] text-[var(--text-secondary)] font-medium">
                          Group by domain name
                        </span>
                      </label>
                    </div>
                  </div>

                  {/* Option 2: Fixed Folder */}
                  <div
                    onClick={() => handleFolderModeSwitch("fixed")}
                    className={`p-3.5 rounded-lg border transition-colors cursor-pointer flex flex-col justify-between ${
                      settings.storage.folderMode === "fixed"
                        ? "bg-[var(--bg-active)] border-[var(--accent)]"
                        : "bg-[var(--bg-chassis)] border-[var(--border-dim)] hover:border-[var(--border-subtle)]"
                    }`}
                  >
                    <div>
                      <div className="flex items-center gap-2">
                        <div
                          className={`w-4 h-4 rounded-full border flex items-center justify-center shrink-0 ${
                            settings.storage.folderMode === "fixed"
                              ? "border-[var(--accent)] bg-[var(--accent)]"
                              : "border-[var(--border-subtle)] bg-[var(--bg-card)]"
                          }`}
                        >
                          {settings.storage.folderMode === "fixed" && <div className="w-1.5 h-1.5 rounded-full bg-white" />}
                        </div>
                        <span className="font-semibold text-xs text-[var(--text-primary)]">
                          Fixed Target Directory
                        </span>
                      </div>
                      <p className="text-[11px] text-[var(--text-muted)] mt-2">
                        Saves all incoming files directly into a specific designated folder without subdirectories.
                      </p>
                    </div>

                    <div className="mt-3 pt-3 border-t border-[var(--border-dim)] space-y-2" onClick={(e) => e.stopPropagation()}>
                      <div className="flex items-center gap-2">
                        <div className="flex-1 relative">
                          <Folder className="w-3.5 h-3.5 absolute left-2.5 top-1/2 -translate-y-1/2 text-[var(--text-muted)] pointer-events-none" />
                          <input
                            type="text"
                            value={settings.downloads.defaultDirectory}
                            onChange={(e) => update("downloads", { defaultDirectory: e.target.value })}
                            className="w-full h-8 pl-8 pr-2.5 rounded-lg text-xs font-mono bg-[var(--bg-card)] border border-[var(--border-subtle)] text-[var(--text-primary)] focus:outline-none focus:border-[var(--accent)]"
                          />
                        </div>
                        <button
                          onClick={handleBrowseFolder}
                          className="h-8 px-2.5 rounded-lg border border-[var(--border-subtle)] bg-transparent hover:bg-white/[0.05] text-xs font-medium text-[var(--text-secondary)] hover:text-[var(--text-primary)] inline-flex items-center gap-1.5 cursor-pointer shrink-0 transition-colors"
                          title="Choose folder"
                        >
                          <FolderOpen className="w-3.5 h-3.5 text-[var(--accent)]" />
                          <span>Browse</span>
                        </button>
                      </div>

                      <div className="flex items-center justify-between text-[10px] text-[var(--text-muted)] font-mono pt-0.5">
                        <div className="flex items-center gap-1.5">
                          <HardDrive className="w-3 h-3 text-emerald-400" />
                          <span>Volume ({diskSpace?.drive || "C:"})</span>
                        </div>
                        <span className="text-emerald-400 font-semibold">
                          {diskSpace && diskSpace.total_bytes > 0 ? `${diskSpace.pct_free}% Free` : "Online"}
                        </span>
                      </div>
                    </div>
                  </div>
                </div>

                {/* Sub-option: Ask Every Time */}
                <div className="px-4 py-3 flex items-center justify-between gap-4">
                  <div>
                    <span className="font-medium text-xs text-[var(--text-primary)] block">Pre-Download Location Confirmation</span>
                    <span className="text-[11px] text-[var(--text-muted)]">Always display confirmation dialog to confirm target directory and filename before starting.</span>
                  </div>
                  <input
                    type="checkbox"
                    checked={settings.storage.askForSaveLocation}
                    onChange={(e) => update("storage", { askForSaveLocation: e.target.checked })}
                    className="edm-checkbox"
                  />
                </div>
              </div>

              {/* Filename Conflict Policy */}
              <div className="rounded-xl border border-[var(--border-dim)] bg-[var(--bg-card)] p-4 space-y-3">
                <label className="text-xs font-semibold text-[var(--text-primary)] block">
                  File Collision &amp; Conflict Policy
                </label>
                <div className="grid grid-cols-2 sm:grid-cols-4 gap-2">
                  {[
                    { id: "ask" as const, label: "Ask Every Time", desc: "Show resolution dialog" },
                    { id: "rename" as const, label: "Auto-Rename (1)", desc: "Append numerical counter" },
                    { id: "overwrite" as const, label: "Overwrite File", desc: "Replace target completely" },
                    { id: "skip" as const, label: "Skip Download", desc: "Ignore duplicate tasks" },
                  ].map((p) => {
                    const isSel = settings.storage.filenameConflictPolicy === p.id;
                    return (
                      <button
                        key={p.id}
                        onClick={() => update("storage", { filenameConflictPolicy: p.id })}
                        className={`p-2.5 rounded-lg border text-left cursor-pointer transition-colors ${
                          isSel
                            ? "bg-[var(--bg-active)] border-[var(--accent)] font-semibold"
                            : "bg-[var(--bg-chassis)] border-[var(--border-dim)] hover:border-[var(--border-subtle)]"
                        }`}
                      >
                        <div className="text-xs text-[var(--text-primary)]">{p.label}</div>
                        <div className="text-[10px] text-[var(--text-muted)] mt-0.5">{p.desc}</div>
                      </button>
                    );
                  })}
                </div>
              </div>

              {/* Multi-Stream Concurrency & Segment Acceleration */}
              <div className="rounded-xl border border-[var(--border-dim)] bg-[var(--bg-card)] divide-y divide-[var(--border-dim)]">
                {/* Auto-Adaptive Engine Toggle */}
                <div className="px-4 py-3 flex items-center justify-between gap-4">
                  <div className="space-y-0.5">
                    <div className="flex items-center gap-2">
                      <span className="font-semibold text-xs text-[var(--text-primary)]">
                        Auto-Adaptive Engine
                      </span>
                      <span className="px-1.5 py-0.5 rounded text-[10px] font-semibold bg-emerald-500/15 text-emerald-400 border border-emerald-500/30">
                        Recommended
                      </span>
                    </div>
                    <span className="text-[11px] text-[var(--text-muted)] block">
                      Automatic HTTP/2 vs HTTP/1.1 protocol detection, dynamic BDP stream scaling, work-stealing, and speculative tail hedging.
                    </span>
                  </div>
                  <input
                    type="checkbox"
                    checked={settings.downloads.autoAdaptiveStreams ?? true}
                    onChange={(e) => update("downloads", { autoAdaptiveStreams: e.target.checked })}
                    className="edm-checkbox shrink-0"
                  />
                </div>

                <div className="p-4 space-y-3">
                  <label className="text-xs font-semibold text-[var(--text-primary)] block">
                    Multi-Stream Acceleration &amp; Concurrency Limits
                  </label>

                  <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                    {/* Active Concurrency */}
                    <div className="p-3 rounded-lg bg-[var(--bg-chassis)] border border-[var(--border-dim)] space-y-2">
                      <div className="flex justify-between items-center text-xs">
                        <span className="font-medium text-[var(--text-secondary)]">Max Simultaneous Tasks</span>
                        <span className="font-mono font-bold text-[var(--accent)] text-sm">
                          {settings.downloads.maxActiveDownloads} Downloads
                        </span>
                      </div>
                      <input
                        type="range"
                        min={1}
                        max={10}
                        value={settings.downloads.maxActiveDownloads}
                        onChange={(e) => update("downloads", { maxActiveDownloads: Number(e.target.value) })}
                        className="w-full accent-[var(--accent)] cursor-pointer"
                      />
                      <div className="flex justify-between text-[10px] text-[var(--text-muted)] font-mono">
                        <span>1 (Sequential)</span>
                        <span>5</span>
                        <span>10 (Maximum)</span>
                      </div>
                    </div>

                    {/* Max Connection Ceiling */}
                    <div className="p-3 rounded-lg bg-[var(--bg-chassis)] border border-[var(--border-dim)] space-y-2">
                      <div className="flex justify-between items-center text-xs">
                        <span className="font-medium text-[var(--text-secondary)]">Max Connection Ceiling</span>
                        <span className="font-mono font-bold text-[var(--accent)] text-sm">
                          Up to {Math.max(4, Math.min(64, settings.downloads.maxConnectionsPerFile || 32))} Streams
                        </span>
                      </div>
                      <input
                        type="range"
                        min={4}
                        max={64}
                        value={Math.max(4, Math.min(64, settings.downloads.maxConnectionsPerFile || 32))}
                        onChange={(e) => update("downloads", { maxConnectionsPerFile: Number(e.target.value) })}
                        className="w-full accent-[var(--accent)] cursor-pointer"
                      />
                      <div className="flex justify-between text-[10px] text-[var(--text-muted)] font-mono">
                        <span>4 Max</span>
                        <span>32 (Default)</span>
                        <span>64 Max</span>
                      </div>
                    </div>
                  </div>
                </div>

                {/* Segmentation Strategy Segmented Control */}
                <div className="p-4 space-y-2.5">
                  <label className="text-xs font-semibold text-[var(--text-primary)] block">
                    Dynamic Byte-Range Splitting Strategy
                  </label>
                  <div className="grid grid-cols-1 sm:grid-cols-3 gap-2">
                    {[
                      {
                        id: "automatic" as const,
                        label: "Automatic (Dynamic Range)",
                        desc: "Splits into byte-ranges when HTTP 206 advertised",
                      },
                      {
                        id: "aggressive" as const,
                        label: "Aggressive Multi-Stream",
                        desc: "Immediately probes and partitions chunks",
                      },
                      {
                        id: "disabled" as const,
                        label: "Single Stream Only",
                        desc: "Strict 1-thread mode for fragile servers",
                      },
                    ].map((mode) => (
                      <button
                        key={mode.id}
                        onClick={() => update("downloads", { segmentedDownloads: mode.id })}
                        className={`p-2.5 rounded-lg border text-left cursor-pointer transition-colors ${
                          settings.downloads.segmentedDownloads === mode.id
                            ? "bg-[var(--bg-active)] border-[var(--accent)] font-semibold"
                            : "bg-[var(--bg-chassis)] border-[var(--border-dim)] hover:border-[var(--border-subtle)]"
                        }`}
                      >
                        <div className="text-xs text-[var(--text-primary)]">{mode.label}</div>
                        <div className="text-[10px] text-[var(--text-muted)] mt-0.5">{mode.desc}</div>
                      </button>
                    ))}
                  </div>
                </div>
              </div>

              {/* Smart Category Rules Table */}
              <div className="rounded-xl border border-[var(--border-dim)] bg-[var(--bg-card)] p-4 space-y-3">
                <div className="flex items-center justify-between">
                  <label className="text-xs font-semibold text-[var(--text-primary)] block">
                    Extension-to-Folder Mapping Rules
                  </label>
                  <span className="text-[11px] text-[var(--text-muted)]">
                    {settings.categories.length} Registered Categories
                  </span>
                </div>

                <div className="rounded-lg border border-[var(--border-dim)] overflow-x-auto bg-[var(--bg-chassis)]">
                  <table className="w-full text-left text-xs table-fixed min-w-[600px]">
                    <thead className="bg-[var(--bg-card)] border-b border-[var(--border-dim)] text-[10px] font-semibold uppercase tracking-wider text-[var(--text-muted)]">
                      <tr>
                        <th className="py-2 px-3 w-12 text-center">Active</th>
                        <th className="py-2 px-3 w-28">Category</th>
                        <th className="py-2 px-3 w-5/12">Extensions</th>
                        <th className="py-2 px-3">Destination Path</th>
                        <th className="py-2 px-3 w-14 text-right">Action</th>
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-[var(--border-dim)] font-mono text-[11px]">
                      {settings.categories.map((cat) => (
                        <tr key={cat.id} className="hover:bg-[var(--bg-card-hover)]">
                          <td className="py-2 px-3 w-12 text-center">
                            <input
                              type="checkbox"
                              checked={cat.enabled}
                              onChange={(e) => {
                                const updatedCats = settings.categories.map((c) =>
                                  c.id === cat.id ? { ...c, enabled: e.target.checked } : c
                                );
                                onUpdateSettings({ ...settings, categories: updatedCats });
                                saveSettings({ ...settings, categories: updatedCats });
                              }}
                              className="w-4 h-4 rounded accent-[var(--accent)] cursor-pointer"
                            />
                          </td>
                          <td className="py-2 px-3 w-28 font-semibold text-[var(--text-primary)]">{cat.name}</td>
                          <td className="py-2 px-3 w-5/12">
                            <div className="flex flex-wrap gap-1.5 items-center">
                              {cat.extensions.map((ext) => (
                                <span
                                  key={ext}
                                  className="px-1 py-0.5 rounded bg-[var(--bg-card)] border border-[var(--border-dim)] text-[10px]"
                                >
                                  .{ext}
                                </span>
                              ))}
                            </div>
                          </td>
                          <td className="py-2 px-3 text-[var(--text-muted)]">
                            <div className="flex items-center justify-between gap-1.5">
                              <span className="truncate flex-1 font-mono text-[11px]" title={cat.destinationDirectory || "Inherit default"}>
                                {cat.destinationDirectory || "(Inherit default)"}
                              </span>
                              <button
                                onClick={() => handleBrowseCategoryFolder(cat.id, cat.destinationDirectory)}
                                className="p-1 rounded hover:bg-white/10 text-[var(--text-muted)] hover:text-[var(--accent)] transition-colors shrink-0 cursor-pointer"
                                title="Browse folder..."
                              >
                                <FolderOpen className="w-3.5 h-3.5" />
                              </button>
                            </div>
                          </td>
                          <td className="py-2 px-3 w-14 text-right">
                            <button
                              onClick={() => handleDeleteCategory(cat.id)}
                              className="p-1 rounded text-[var(--text-muted)] hover:text-rose-400 hover:bg-rose-500/10 cursor-pointer"
                              title="Delete Rule"
                            >
                              <Trash2 className="w-3.5 h-3.5" />
                            </button>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>

                {/* Add Custom Rule Strip */}
                <div className="flex items-center gap-2 pt-1">
                  <input
                    type="text"
                    placeholder="New Category (e.g. Torrents)"
                    value={customCatName}
                    onChange={(e) => setCustomCatName(e.target.value)}
                    className="flex-1 h-8 px-2.5 rounded-lg text-xs bg-[var(--bg-chassis)] border border-[var(--border-subtle)] text-[var(--text-primary)] focus:outline-none focus:border-[var(--accent)]"
                  />
                  <input
                    type="text"
                    placeholder="Extensions (e.g. torrent, magnet)"
                    value={customCatExt}
                    onChange={(e) => setCustomCatExt(e.target.value)}
                    className="flex-1 h-8 px-2.5 rounded-lg text-xs bg-[var(--bg-chassis)] border border-[var(--border-subtle)] text-[var(--text-primary)] focus:outline-none focus:border-[var(--accent)]"
                  />
                  <button
                    onClick={handleAddCategory}
                    className="h-8 px-3 rounded-lg border border-[var(--border-subtle)] bg-transparent hover:bg-white/[0.05] text-xs font-medium text-[var(--text-primary)] inline-flex items-center gap-1 cursor-pointer shrink-0 transition-colors"
                  >
                    <Plus className="w-3.5 h-3.5 text-[var(--accent)]" />
                    <span>Add Rule</span>
                  </button>
                </div>
              </div>
            </section>
          )}

          {/* ═════════════════════════════════════════════════════════
              SECTION 3: FEATURE PACKS & PROTOCOL ENGINES
          ══════════════════════════════════════════════════════════ */}
          {isSectionVisible("packs") && (
            <section id="section-packs" className="space-y-5">
              <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
                <div>
                  <div className="flex items-center gap-2">
                    <Package className="w-4 h-4 text-[var(--accent)]" />
                    <h2 className="text-sm font-semibold text-[var(--text-primary)]">Feature Packs &amp; Protocol Engines</h2>
                  </div>
                  <p className="text-xs text-[var(--text-muted)] mt-1">
                    Multi-protocol adapters, P2P swarm engines, stream demuxers, and accelerated mirror resolvers.
                  </p>
                </div>
                <span className="text-xs font-medium text-[var(--text-muted)] shrink-0">
                  {FEATURE_PACKS.length} Integrated Packs
                </span>
              </div>

              {/* Feature Packs Grid (Proportional radii: outer rounded-xl 12px, padding p-2 8px -> inner rounded 4px) */}
              <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-2.5">
                {FEATURE_PACKS.map((pack) => (
                  <div
                    key={pack.id}
                    className="rounded-xl border border-[var(--border-dim)] bg-[var(--bg-card)] p-2 flex items-center justify-between gap-2.5 min-w-0"
                  >
                    <div className="flex items-center gap-2.5 min-w-0">
                      <div className="p-1.5 rounded bg-[var(--bg-chassis)] border border-[var(--border-dim)] shrink-0">
                        {pack.id === "http_pack" && <Globe className="w-4 h-4 text-blue-400" />}
                        {pack.id === "bittorrent_pack" && <Share2 className="w-4 h-4 text-emerald-400" />}
                        {pack.id === "ftp_pack" && <Server className="w-4 h-4 text-amber-400" />}
                        {pack.id === "ed2k_pack" && <Radio className="w-4 h-4 text-purple-400" />}
                        {pack.id === "m3u8_pack" && <Film className="w-4 h-4 text-cyan-400" />}
                        {pack.id === "ffmpeg_pack" && <Layers className="w-4 h-4 text-pink-400" />}
                        {pack.id === "yt_dlp_pack" && <Sparkles className="w-4 h-4 text-rose-400" />}
                        {pack.id === "github_pack" && <Package className="w-4 h-4 text-slate-300" />}
                        {pack.id === "huggingface_pack" && <Cpu className="w-4 h-4 text-yellow-400" />}
                      </div>
                      <span className="text-xs font-medium text-[var(--text-primary)] truncate" title={pack.name}>
                        {pack.name}
                      </span>
                    </div>
                    <span className="shrink-0 font-mono text-[10px] font-medium text-[var(--accent-badge-text)] bg-[var(--accent-badge-bg)] px-1.5 py-0.5 rounded border border-[var(--accent-badge-border)]">
                      v{pack.version}
                    </span>
                  </div>
                ))}
              </div>
            </section>
          )}

          {/* ═════════════════════════════════════════════════════════
              SECTION 4: NETWORK & BROWSER
          ══════════════════════════════════════════════════════════ */}
          {isSectionVisible("network") && (
            <section id="section-network" className="space-y-5">
              <div>
                <div className="flex items-center gap-2">
                  <Globe className="w-4 h-4 text-[var(--accent)]" />
                  <h2 className="text-sm font-semibold text-[var(--text-primary)]">Network &amp; Browser Integration</h2>
                </div>
                <p className="text-xs text-[var(--text-muted)] mt-1">
                  Browser extension companion status, clipboard URL sniffer, proxy gateways, and connection timeouts.
                </p>
              </div>

              {/* Browser Extension Companion Status */}
              <div className="rounded-xl border border-[var(--border-dim)] bg-[var(--bg-card)] divide-y divide-[var(--border-dim)]">
                <div className="p-4 flex items-center justify-between gap-3">
                  <div>
                    <label className="text-xs font-semibold text-[var(--text-primary)] block">
                      Browser Interception Engine
                    </label>
                    <span className="text-[11px] text-[var(--text-muted)]">
                      Connects Chromium, Edge, Brave, and Firefox via Native Messaging Host.
                    </span>
                  </div>
                  <div className="flex items-center gap-1.5 px-2.5 py-1 rounded-full text-[10px] font-mono font-medium bg-emerald-500/15 text-emerald-400 border border-emerald-500/30 shrink-0">
                    <span className="w-1.5 h-1.5 rounded-full bg-emerald-400" />
                    Host v{appVersion} Ready
                  </div>
                </div>

                <div className="px-4 py-3 flex items-center justify-between gap-4">
                  <div>
                    <span className="font-medium text-xs text-[var(--text-primary)] block">Auto-Intercept Browser Downloads</span>
                    <span className="text-[11px] text-[var(--text-muted)]">Capture single-click and right-click download events from connected browsers.</span>
                  </div>
                  <input
                    type="checkbox"
                    checked={settings.browser.integrationEnabled}
                    onChange={(e) => update("browser", { integrationEnabled: e.target.checked })}
                    className="edm-checkbox"
                  />
                </div>

                <div className="px-4 py-3 flex items-center justify-between gap-4">
                  <div>
                    <span className="font-medium text-xs text-[var(--text-primary)] block">Streaming Media Sniffer</span>
                    <span className="text-[11px] text-[var(--text-muted)]">Sniff and capture HLS (.m3u8), MPEG-DASH, and direct video/audio URLs.</span>
                  </div>
                  <input
                    type="checkbox"
                    checked={settings.browser.interceptMediaStreams}
                    onChange={(e) => update("browser", { interceptMediaStreams: e.target.checked })}
                    className="edm-checkbox"
                  />
                </div>

                <div className="px-4 py-3 flex items-center justify-between gap-4">
                  <div>
                    <span className="font-medium text-xs text-[var(--text-primary)] block">YouTube Quick Download Button</span>
                    <span className="text-[11px] text-[var(--text-muted)]">Show quick download button next to Subscribe on YouTube watch pages.</span>
                  </div>
                  <input
                    type="checkbox"
                    checked={settings.browser.youtubeDownloadButtonEnabled ?? true}
                    onChange={(e) => update("browser", { youtubeDownloadButtonEnabled: e.target.checked })}
                    className="edm-checkbox"
                  />
                </div>

                <div className="px-4 py-3 flex items-center justify-between gap-4">
                  <div>
                    <span className="font-medium text-xs text-[var(--text-primary)] block">Clipboard URL Monitoring</span>
                    <span className="text-[11px] text-[var(--text-muted)]">Automatically prompt to download when copying media or file URLs.</span>
                  </div>
                  <input
                    type="checkbox"
                    checked={settings.browser.clipboardMonitoringEnabled}
                    onChange={(e) => update("browser", { clipboardMonitoringEnabled: e.target.checked })}
                    className="edm-checkbox"
                  />
                </div>

                {/* Local WebSocket Bridge Token */}
                <div className="p-4 space-y-2.5">
                  <div className="flex items-center justify-between gap-2">
                    <div className="flex items-center gap-2">
                      <Key className="w-3.5 h-3.5 text-[var(--accent)]" />
                      <span className="text-xs font-semibold text-[var(--text-primary)]">
                        Extension WebSocket Authentication Token
                      </span>
                    </div>
                    <div className="flex items-center gap-1.5 px-2 py-0.5 rounded-full text-[10px] font-mono font-medium bg-cyan-500/15 text-cyan-400 border border-cyan-500/30">
                      <span className="w-1.5 h-1.5 rounded-full bg-cyan-400" />
                      ws://127.0.0.1:14370
                    </div>
                  </div>

                  <p className="text-[11px] text-[var(--text-muted)] leading-relaxed">
                    This security token authenticates your browser extension with the desktop WebSocket engine for live progress synchronization.
                  </p>

                  <div className="flex items-center gap-2 pt-0.5">
                    <input
                      type="text"
                      readOnly
                      value={wsToken || "Loading token..."}
                      className="flex-1 h-8 px-2.5 rounded-lg text-xs font-mono bg-[var(--bg-chassis)] border border-[var(--border-subtle)] text-[var(--text-primary)] focus:outline-none select-all"
                    />
                    <motion.button
                      whileTap={{ scale: 0.97 }}
                      transition={springs.snappy}
                      onClick={handleCopyToken}
                      className="h-8 px-3 rounded-lg border border-[var(--border-subtle)] bg-transparent hover:bg-white/[0.05] text-xs font-medium text-[var(--text-primary)] inline-flex items-center gap-1.5 cursor-pointer shrink-0 transition-colors"
                    >
                      {copiedToken ? (
                        <>
                          <Check className="w-3.5 h-3.5 text-emerald-400" />
                          <span className="text-emerald-400">Copied!</span>
                        </>
                      ) : (
                        <>
                          <Copy className="w-3.5 h-3.5" />
                          <span>Copy Token</span>
                        </>
                      )}
                    </motion.button>
                  </div>
                </div>
              </div>

              {/* Proxy & Protocol */}
              <div className="rounded-xl border border-[var(--border-dim)] bg-[var(--bg-card)] p-4 space-y-3">
                <label className="text-xs font-semibold text-[var(--text-primary)] block">Proxy Configuration</label>

                <div className="grid grid-cols-1 sm:grid-cols-3 gap-2">
                  {[
                    { id: "direct" as const, label: "Direct Connection", desc: "No proxy intermediary" },
                    { id: "system" as const, label: "Use System Proxy", desc: "Inherit OS network settings" },
                    { id: "manual" as const, label: "Manual Proxy", desc: "Custom SOCKS5 / HTTP proxy" },
                  ].map((p) => (
                    <button
                      key={p.id}
                      onClick={() => update("network", { proxyMode: p.id })}
                      className={`p-2.5 rounded-lg border text-left cursor-pointer transition-colors ${
                        settings.network.proxyMode === p.id
                          ? "bg-[var(--bg-active)] border-[var(--accent)] font-semibold"
                          : "bg-[var(--bg-chassis)] border-[var(--border-dim)] hover:border-[var(--border-subtle)]"
                      }`}
                    >
                      <div className="text-xs text-[var(--text-primary)]">{p.label}</div>
                      <div className="text-[10px] text-[var(--text-muted)] mt-0.5">{p.desc}</div>
                    </button>
                  ))}
                </div>

                {settings.network.proxyMode === "manual" && (
                  <div className="p-3 rounded-lg bg-[var(--bg-chassis)] border border-[var(--border-dim)] grid grid-cols-1 sm:grid-cols-3 gap-2">
                    <div>
                      <label className="text-[10px] font-semibold text-[var(--text-muted)] uppercase block mb-1">
                        Protocol
                      </label>
                      <select
                        value={settings.network.proxyProtocol}
                        onChange={(e) => update("network", { proxyProtocol: e.target.value as any })}
                        className="w-full h-8 px-2 rounded-lg bg-[var(--bg-card)] border border-[var(--border-subtle)] text-xs text-[var(--text-primary)]"
                      >
                        <option value="socks5">SOCKS5</option>
                        <option value="http">HTTP</option>
                        <option value="https">HTTPS</option>
                      </select>
                    </div>
                    <div>
                      <label className="text-[10px] font-semibold text-[var(--text-muted)] uppercase block mb-1">
                        Proxy Host
                      </label>
                      <input
                        type="text"
                        value={settings.network.proxyHost}
                        onChange={(e) => update("network", { proxyHost: e.target.value })}
                        className="w-full h-8 px-2 rounded-lg bg-[var(--bg-card)] border border-[var(--border-subtle)] text-xs font-mono text-[var(--text-primary)]"
                      />
                    </div>
                    <div>
                      <label className="text-[10px] font-semibold text-[var(--text-muted)] uppercase block mb-1">
                        Port
                      </label>
                      <input
                        type="number"
                        value={settings.network.proxyPort || ""}
                        onChange={(e) => update("network", { proxyPort: Number(e.target.value) || null })}
                        className="w-full h-8 px-2 rounded-lg bg-[var(--bg-card)] border border-[var(--border-subtle)] text-xs font-mono text-[var(--text-primary)]"
                      />
                    </div>
                  </div>
                )}
              </div>

              <RoutingAndCategoryPanel />
            </section>
          )}

          {/* ═════════════════════════════════════════════════════════
              SECTION 5: ALERTS & NOTIFICATIONS
          ══════════════════════════════════════════════════════════ */}
          {isSectionVisible("automation") && (
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

              {/* Desktop Notifications */}
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
          )}

          {/* ═════════════════════════════════════════════════════════
              SECTION 6: SECURITY & ENGINE
          ══════════════════════════════════════════════════════════ */}
          {isSectionVisible("engine") && (
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

              {/* Security */}
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

              {/* Engine Tuning */}
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

              {/* Global Shortcuts Matrix */}
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
          )}

          {/* ═════════════════════════════════════════════════════════
              SECTION 7: ABOUT & MAINTENANCE
          ══════════════════════════════════════════════════════════ */}
          {isSectionVisible("about") && (
            <section id="section-about" className="space-y-5">
              <div>
                <div className="flex items-center gap-2">
                  <Info className="w-4 h-4 text-[var(--accent)]" />
                  <h2 className="text-sm font-semibold text-[var(--text-primary)]">About &amp; Maintenance</h2>
                </div>
                <p className="text-xs text-[var(--text-muted)] mt-1">
                  Engine telemetry, version specifications, configuration backup/restore, and reset operations.
                </p>
              </div>

              {/* Version & Build Spec */}
              <div className="rounded-xl border border-[var(--border-dim)] bg-[var(--bg-card)] p-4 flex items-center justify-between gap-4">
                <div className="flex items-center gap-3">
                  <img src="/logo.png" alt="EDM Logo" className="w-9 h-9 object-contain" />
                  <div>
                    <div className="flex items-center gap-2">
                      <span className="font-semibold text-sm text-[var(--text-primary)]">Evo Download Manager</span>
                      <span className="px-1.5 py-0.5 rounded text-[10px] font-mono font-semibold bg-[var(--accent-badge-bg)] text-[var(--accent-badge-text)] border border-[var(--accent-badge-border)]">
                        v{appVersion}
                      </span>
                    </div>
                    <p className="text-[11px] text-[var(--text-muted)] mt-0.5 font-mono">
                      x86_64-pc-windows-msvc • Tokio 1.40 • SQLite 3.46 WAL
                    </p>
                  </div>
                </div>
                <div className="text-right text-[11px] font-mono text-[var(--text-muted)] hidden sm:block">
                  <div>Tauri 2.0 Native Shell</div>
                  <div className="text-emerald-400">Zero Telemetry Leaks</div>
                </div>
              </div>

              {/* Software Updates & Check for Updates */}
              <div className="rounded-xl border border-[var(--border-dim)] bg-[var(--bg-card)] divide-y divide-[var(--border-dim)]">
                <div className="p-4 space-y-3.5">
                  <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
                    <div>
                      <label className="text-xs font-semibold text-[var(--text-primary)] block">
                        Software Updates &amp; Pipeline
                      </label>
                      <span className="text-[11px] text-[var(--text-muted)]">
                        {updateStatus === "checking"
                          ? "Checking upstream GitHub releases and signature manifests..."
                          : updateStatus === "up-to-date"
                          ? `EDM is up to date (v${appVersion}). You are running the latest production build.`
                          : updateStatus === "update-available"
                          ? `A newer release (v${updateInfo?.latest_version}) is available for download!`
                          : updateStatus === "downloading"
                          ? `Downloading update package... ${otaProgress ? `${otaProgress.percent.toFixed(0)}% (${formatBytes(otaProgress.downloaded_bytes)} / ${formatBytes(otaProgress.total_bytes)})` : ""}`
                          : updateStatus === "ready"
                          ? "Download complete. Launching installer and restarting EDM..."
                          : updateStatus === "error"
                          ? `Update check failed: ${updateError || "Network error"}`
                          : `Current channel: Stable. Last checked: ${lastCheckedTime}`}
                      </span>
                    </div>

                    <div className="flex items-center gap-2 shrink-0">
                      {updateStatus === "up-to-date" && (
                        <span className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-semibold bg-emerald-500/15 text-emerald-400 border border-emerald-500/30">
                          <CheckCircle2 className="w-3.5 h-3.5 text-emerald-400" />
                          Up to Date
                        </span>
                      )}

                      {updateStatus === "update-available" && (
                        <button
                          onClick={handleInstallUpdate}
                          className="h-8 px-3 rounded-lg text-xs font-semibold inline-flex items-center gap-1.5 cursor-pointer bg-emerald-600 hover:bg-emerald-500 text-white transition-colors"
                          title="Download and install update automatically"
                        >
                          <Download className="w-3.5 h-3.5" />
                          <span>Update Now</span>
                        </button>
                      )}

                      {updateStatus === "error" && (
                        <div className="flex items-center gap-1.5">
                          <span className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-semibold bg-red-500/15 text-red-400 border border-red-500/30">
                            <AlertTriangle className="w-3.5 h-3.5 text-red-400" />
                            Failed
                          </span>
                          <button
                            onClick={handleInstallUpdate}
                            className="h-8 px-2.5 rounded-lg text-xs font-medium inline-flex items-center gap-1 cursor-pointer bg-amber-500/15 text-amber-400 border border-amber-500/30 hover:bg-amber-500/25 transition-colors"
                            title="Retry update installation"
                          >
                            <RefreshCw className="w-3 h-3" />
                            <span>Retry</span>
                          </button>
                          <button
                            onClick={() => tauriOpenInstallerFolder()}
                            className="h-8 px-2.5 rounded-lg text-xs font-medium inline-flex items-center gap-1 cursor-pointer bg-blue-500/15 text-blue-400 border border-blue-500/30 hover:bg-blue-500/25 transition-colors"
                            title="Open installer directory in File Explorer"
                          >
                            <FolderOpen className="w-3 h-3" />
                            <span>Open folder</span>
                          </button>
                        </div>
                      )}

                      <button
                        onClick={handleCheckUpdate}
                        disabled={updateStatus === "checking" || updateStatus === "downloading"}
                        className="h-8 px-3 rounded-lg border border-[var(--border-subtle)] bg-transparent hover:bg-white/[0.05] text-xs font-medium text-[var(--text-primary)] inline-flex items-center gap-1.5 cursor-pointer disabled:opacity-50 transition-colors"
                        title="Check for newer EDM versions on GitHub"
                      >
                        <RefreshCw
                          className={`w-3.5 h-3.5 text-[var(--accent)] ${
                            updateStatus === "checking" ? "animate-spin" : ""
                          }`}
                        />
                        <span>
                          {updateStatus === "checking"
                            ? "Checking..."
                            : updateStatus === "downloading"
                            ? "Downloading..."
                            : "Check for Updates"}
                        </span>
                      </button>
                    </div>
                  </div>

                  {/* Download Progress Bar */}
                  {(updateStatus === "downloading" || updateStatus === "ready") && (
                    <div className="p-3 rounded-lg bg-[var(--bg-chassis)] border border-[var(--border-dim)] space-y-2">
                      <div className="flex items-center justify-between text-xs">
                        <span className="font-semibold text-[var(--text-primary)]">
                          {updateStatus === "ready" ? "Installer Ready" : "Downloading Over-the-Air Update"}
                        </span>
                        <span className="font-mono text-[var(--accent)] font-bold">
                          {otaProgress ? `${otaProgress.percent.toFixed(0)}%` : "0%"}
                        </span>
                      </div>
                      <div className="w-full h-2 rounded-full bg-[var(--bg-card)] overflow-hidden border border-[var(--border-dim)]">
                        <div
                          className="h-full bg-gradient-to-r from-[var(--accent)] to-emerald-400 transition-all duration-150"
                          style={{ width: `${otaProgress ? otaProgress.percent : 0}%` }}
                        />
                      </div>
                      <div className="flex items-center justify-between text-[11px] text-[var(--text-muted)] font-mono">
                        <span>
                          {otaProgress
                            ? `${formatBytes(otaProgress.downloaded_bytes)} of ${formatBytes(otaProgress.total_bytes)}`
                            : "Connecting..."}
                        </span>
                        <span>Target: {updateInfo?.asset_name || "EDM Setup"}</span>
                      </div>
                    </div>
                  )}

                  {/* Update Available Detail */}
                  {updateStatus === "update-available" && updateInfo && (
                    <div className="p-3.5 rounded-lg bg-[var(--bg-chassis)] border border-emerald-500/30 space-y-2">
                      <div className="flex items-center justify-between">
                        <div className="flex items-center gap-2">
                          <Sparkles className="w-4 h-4 text-emerald-400" />
                          <span className="text-xs font-semibold text-[var(--text-primary)]">
                            {updateInfo.release_name}
                          </span>
                          <span className="px-1.5 py-0.5 rounded text-[10px] font-mono font-semibold bg-emerald-500/20 text-emerald-400">
                            v{updateInfo.latest_version}
                          </span>
                        </div>
                        <a
                          href={updateInfo.html_url}
                          target="_blank"
                          rel="noreferrer"
                          className="text-[11px] text-[var(--accent)] hover:underline inline-flex items-center gap-1 font-medium"
                        >
                          <span>Release Notes</span>
                          <ExternalLink className="w-3 h-3" />
                        </a>
                      </div>
                      {updateInfo.asset_name && (
                        <div className="text-[11px] text-[var(--text-muted)] font-mono flex items-center gap-2">
                          <span>Package: {updateInfo.asset_name}</span>
                          <span>•</span>
                          <span>Size: {formatBytes(updateInfo.asset_size)}</span>
                        </div>
                      )}
                    </div>
                  )}

                  {/* Dedicated Reinstall & Repair Mode trigger */}
                  <div className="text-right">
                    <button
                      onClick={() => {
                        window.location.hash = "#repair";
                        window.location.reload();
                      }}
                      className="text-[11px] text-[var(--text-muted)] hover:text-cyan-400 underline cursor-pointer inline-flex items-center gap-1"
                      title="Launch dedicated system repair utility"
                    >
                      <Wrench className="w-3 h-3 text-cyan-400" />
                      <span>Reinstall / Repair v{appVersion}</span>
                    </button>
                  </div>
                </div>

                <div className="px-4 py-3 flex flex-col sm:flex-row sm:items-center justify-between gap-2">
                  <label className="flex items-center gap-3 cursor-pointer group">
                    <input
                      type="checkbox"
                      checked={settings.general.autoCheckUpdates}
                      onChange={(e) => update("general", { autoCheckUpdates: e.target.checked })}
                      className="edm-checkbox"
                    />
                    <span className="text-xs font-medium text-[var(--text-secondary)] group-hover:text-[var(--text-primary)]">
                      Automatically check for updates on application launch
                    </span>
                  </label>

                  <span className="text-[10px] font-mono text-[var(--text-muted)]">
                    Channel: <span className="text-[var(--accent-badge-text)] font-semibold">Stable (Production)</span>
                  </span>
                </div>
              </div>

              {/* Backup & Import Configuration */}
              <div className="rounded-xl border border-[var(--border-dim)] bg-[var(--bg-card)] p-4 space-y-3">
                <label className="text-xs font-semibold text-[var(--text-primary)] block">
                  Configuration Backup &amp; Migration
                </label>
                <div className="flex items-center gap-2.5">
                  <button
                    onClick={handleExport}
                    className="h-8 px-3 rounded-lg border border-[var(--border-subtle)] bg-transparent hover:bg-white/[0.05] text-xs font-medium text-[var(--text-primary)] inline-flex items-center gap-1.5 cursor-pointer transition-colors"
                  >
                    <Download className="w-3.5 h-3.5 text-[var(--accent)]" />
                    <span>Backup Settings (JSON)</span>
                  </button>

                  <label className="h-8 px-3 rounded-lg border border-[var(--border-subtle)] bg-transparent hover:bg-white/[0.05] text-xs font-medium text-[var(--text-primary)] inline-flex items-center gap-1.5 cursor-pointer transition-colors">
                    <Upload className="w-3.5 h-3.5 text-emerald-400" />
                    <span>Restore Settings...</span>
                    <input type="file" accept=".json" onChange={handleImport} className="hidden" />
                  </label>
                </div>
                {importStatus && (
                  <p className={`text-xs font-medium ${importStatus.type === "success" ? "text-emerald-400" : "text-rose-400"}`}>
                    {importStatus.message}
                  </p>
                )}
              </div>

              {/* Danger Zone: Factory Reset */}
              <div className="p-4 rounded-xl border border-rose-500/30 bg-rose-500/5 space-y-2">
                <div className="flex items-center gap-2 text-rose-400 font-semibold text-xs">
                  <AlertTriangle className="w-4 h-4" />
                  <span>Factory Reset</span>
                </div>
                <p className="text-[11px] text-[var(--text-muted)]">
                  Permanently restores all EDM preferences, categories, and network configs to original defaults.
                </p>
                <button
                  onClick={() => {
                    if (confirm("Reset ALL settings to factory defaults? This action cannot be undone.")) {
                      onUpdateSettings(defaultSettings);
                      saveSettings(defaultSettings);
                    }
                  }}
                  className="px-3 py-1.5 rounded-lg text-xs font-semibold bg-rose-500/20 text-rose-400 border border-rose-500/30 hover:bg-rose-500/30 cursor-pointer transition-colors"
                >
                  Factory Reset All Settings
                </button>
              </div>
            </section>
          )}
          </div>
        </main>
      </div>
    </div>
  );
}

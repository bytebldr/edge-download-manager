import type { FeaturePackId } from "./featurePacks";

export interface CategoryRule {
  id: string;
  name: string;
  icon?: string;
  enabled: boolean;
  extensions: string[];
  destinationDirectory: string;
  priority: "high" | "normal" | "low";
}

export interface EdmSettings {
  schemaVersion: number;
  general: {
    theme: "dark" | "light" | "system";
    palette: "blue" | "emerald" | "amber" | "violet" | "rose" | "titanium";
    language: string;
    uiStyle: "pro" | "compact" | "modern";
    launchAtStartup: boolean;
    startMinimized: boolean;
    minimizeToTray: boolean;
    restoreDownloadsOnLaunch: boolean;
    confirmBeforeExit: boolean;
    preventDuplicateInstances: boolean;
    autoCheckUpdates: boolean;
    anonymousAnalytics: boolean;
    confirmDestructiveActions: boolean;
    keepAwake?: boolean;
    idleExitMinutes?: number | null; // null = never, 0 = immediately after listener sleep, 5, 15, 30, 60
    showConnectionBoundaries?: boolean;
  };
  downloads: {
    defaultDirectory: string;
    temporaryDirectory: string;
    maxActiveDownloads: number;
    globalSpeedLimitBytesPerSecond: number | null; // null = unlimited
    maxConnectionsPerFile: number;
    totalConnectionsLimit: number;
    segmentedDownloads: "automatic" | "disabled" | "aggressive";
    maxSegments: number;
    minFileSizeForSegmentationMb: number;
    retryEnabled: boolean;
    maxRetries: number;
    retryInitialDelaySeconds: number;
    retryMaxDelaySeconds: number;
    resumeEnabled: boolean;
  };
  storage: {
    folderMode: "auto" | "fixed";
    suggestFoldersByType: boolean;
    suggestFoldersByDomain: boolean;
    askForSaveLocation: boolean;
    filenameConflictPolicy: "ask" | "rename" | "overwrite" | "skip";
    preservePartialFiles: boolean;
    warnOnLowDiskSpace: boolean;
    lowDiskThresholdMb: number;
    createCategoryFolders: boolean;
    cleanupStaleTempFiles: boolean;
  };
  categories: CategoryRule[];
  browser: {
    integrationEnabled: boolean;
    clipboardMonitoringEnabled: boolean;
    confirmCapturedUrls: boolean;
    interceptMediaStreams: boolean;
    youtubeDownloadButtonEnabled: boolean;
    excludedDomains: string[];
  };
  network: {
    proxyMode: "direct" | "system" | "manual";
    proxyProtocol: "http" | "https" | "socks5";
    proxyHost: string;
    proxyPort: number | null;
    connectionTimeoutSeconds: number;
    readTimeoutSeconds: number;
    ipPreference: "automatic" | "ipv4" | "ipv6";
    enforceTlsCertVerification: boolean;
  };
  queues: {
    defaultQueue: string;
    schedulerEnabled: boolean;
    offPeakStart: string;
    offPeakEnd: string;
    daysOfWeek: number[]; // 0 = Sunday, 1 = Monday, etc.
    postCompletionAction: "none" | "sleep" | "hibernate" | "shutdown";
  };
  notifications: {
    downloadStarted: boolean;
    downloadCompleted: boolean;
    downloadFailed: boolean;
    queueCompleted: boolean;
    lowDiskSpace: boolean;
    playSound: boolean;
  };
  security: {
    verifyChecksums: boolean;
    checksumAlgorithm: "SHA-256" | "MD5" | "SHA-1";
    sanitizeFilenames: boolean;
    redactLogs: boolean;
    blockInsecureHttp: boolean;
  };
  advanced: {
    sqliteJournalMode: "WAL" | "DELETE" | "MEMORY";
    sqliteSyncMode: "NORMAL" | "FULL";
    flushBeforeCheckpoint: boolean;
    socketBufferSizeKb: number;
    logLevel: "debug" | "info" | "warn" | "error";
  };
  featurePacks: Record<FeaturePackId, boolean>;
}

export const defaultSettings: EdmSettings = {
  schemaVersion: 1,
  general: {
    theme: "dark",
    palette: "blue",
    language: "en-US",
    uiStyle: "pro",
    launchAtStartup: false,
    startMinimized: false,
    minimizeToTray: true,
    restoreDownloadsOnLaunch: true,
    confirmBeforeExit: true,
    preventDuplicateInstances: true,
    autoCheckUpdates: true,
    anonymousAnalytics: false,
    confirmDestructiveActions: true,
    keepAwake: false,
    idleExitMinutes: 15,
    showConnectionBoundaries: false,
  },
  downloads: {
    defaultDirectory: "",
    temporaryDirectory: "",
    maxActiveDownloads: 3,
    globalSpeedLimitBytesPerSecond: null,
    maxConnectionsPerFile: 6,
    totalConnectionsLimit: 32,
    segmentedDownloads: "automatic",
    maxSegments: 8,
    minFileSizeForSegmentationMb: 5,
    retryEnabled: true,
    maxRetries: 5,
    retryInitialDelaySeconds: 2,
    retryMaxDelaySeconds: 60,
    resumeEnabled: true,
  },
  storage: {
    folderMode: "auto",
    suggestFoldersByType: true,
    suggestFoldersByDomain: false,
    askForSaveLocation: false,
    filenameConflictPolicy: "ask",
    preservePartialFiles: true,
    warnOnLowDiskSpace: true,
    lowDiskThresholdMb: 2048,
    createCategoryFolders: true,
    cleanupStaleTempFiles: true,
  },
  categories: [
    {
      id: "programs",
      name: "Programs",
      icon: "FileCode",
      enabled: true,
      extensions: ["exe", "msi", "bat", "cmd", "ps1", "sh", "appx", "msix", "apk", "jar", "vbs", "reg", "bin"],
      destinationDirectory: "",
      priority: "high",
    },
    {
      id: "video",
      name: "Video",
      icon: "Film",
      enabled: true,
      extensions: [
        "mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "ts", "mts", "m2ts", "vob", "ogv", "3gp", "3g2", "rm", "rmvb", "divx", "asf"
      ],
      destinationDirectory: "",
      priority: "normal",
    },
    {
      id: "audio",
      name: "Audio",
      icon: "Music",
      enabled: true,
      extensions: [
        "mp3", "wav", "flac", "aac", "ogg", "m4a", "wma", "opus", "aiff", "alac", "mid", "midi", "ape", "ac3", "dts", "amr"
      ],
      destinationDirectory: "",
      priority: "normal",
    },
    {
      id: "pictures",
      name: "Pictures",
      icon: "Image",
      enabled: true,
      extensions: [
        "jpg", "jpeg", "png", "gif", "webp", "avif", "svg", "bmp", "ico", "tiff", "tif", "psd", "ai", "raw", "cr2", "nef", "heic", "heif", "dng"
      ],
      destinationDirectory: "",
      priority: "normal",
    },
    {
      id: "documents",
      name: "Documents",
      icon: "FileText",
      enabled: true,
      extensions: [
        "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "txt", "rtf", "odt", "ods", "odp", "csv", "tsv", "md", "epub", "mobi", "azw3", "djvu", "pages", "numbers", "key"
      ],
      destinationDirectory: "",
      priority: "normal",
    },
    {
      id: "archives",
      name: "Archives",
      icon: "Archive",
      enabled: true,
      extensions: [
        "zip", "rar", "7z", "tar", "gz", "bz2", "xz", "iso", "dmg", "img", "vhd", "vhdx", "tgz", "tbz2", "cab", "cpio", "z", "lzma", "pkg", "deb", "rpm"
      ],
      destinationDirectory: "",
      priority: "normal",
    },
  ],
  browser: {
    integrationEnabled: true,
    clipboardMonitoringEnabled: false,
    confirmCapturedUrls: true,
    interceptMediaStreams: true,
    youtubeDownloadButtonEnabled: true,
    excludedDomains: ["localhost", "127.0.0.1", "bank.example.com"],
  },
  network: {
    proxyMode: "system",
    proxyProtocol: "http",
    proxyHost: "",
    proxyPort: null,
    connectionTimeoutSeconds: 30,
    readTimeoutSeconds: 60,
    ipPreference: "automatic",
    enforceTlsCertVerification: true,
  },
  queues: {
    defaultQueue: "main",
    schedulerEnabled: false,
    offPeakStart: "02:00",
    offPeakEnd: "07:00",
    daysOfWeek: [1, 2, 3, 4, 5],
    postCompletionAction: "none",
  },
  notifications: {
    downloadStarted: false,
    downloadCompleted: true,
    downloadFailed: true,
    queueCompleted: true,
    lowDiskSpace: true,
    playSound: true,
  },
  security: {
    verifyChecksums: true,
    checksumAlgorithm: "SHA-256",
    sanitizeFilenames: true,
    redactLogs: true,
    blockInsecureHttp: false,
  },
  advanced: {
    sqliteJournalMode: "WAL",
    sqliteSyncMode: "NORMAL",
    flushBeforeCheckpoint: true,
    socketBufferSizeKb: 128,
    logLevel: "info",
  },
  featurePacks: {
    http_pack: true,
    bittorrent_pack: true,
    ftp_pack: true,
    ed2k_pack: true,
    m3u8_pack: true,
    ffmpeg_pack: true,
    yt_dlp_pack: true,
    github_pack: true,
    huggingface_pack: true,
  },
};

const STORAGE_KEY = "edm_settings_v1";

export function loadSettings(): EdmSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return defaultSettings;
    const parsed = JSON.parse(raw);

    let categories: CategoryRule[] = parsed.categories?.length ? parsed.categories : defaultSettings.categories;
    categories = categories.map((c) => {
      if (c.destinationDirectory && (c.destinationDirectory.includes("\\EDM\\EDM") || c.destinationDirectory.includes("/EDM/EDM"))) {
        return { ...c, destinationDirectory: c.destinationDirectory.replace(/([\\/]EDM)+$/gi, "\\EDM") };
      }
      return c;
    });

    if (!categories.some((c) => c.id === "pictures")) {
      const picCat = defaultSettings.categories.find((c) => c.id === "pictures");
      if (picCat) categories.push(picCat);
    }

    // Deep merge to ensure all new fields in defaultSettings are present
    return {
      ...defaultSettings,
      ...parsed,
      general: { ...defaultSettings.general, ...parsed.general },
      downloads: { ...defaultSettings.downloads, ...parsed.downloads },
      storage: { ...defaultSettings.storage, ...parsed.storage },
      browser: { ...defaultSettings.browser, ...parsed.browser },
      network: { ...defaultSettings.network, ...parsed.network },
      queues: { ...defaultSettings.queues, ...parsed.queues },
      notifications: { ...defaultSettings.notifications, ...parsed.notifications },
      security: { ...defaultSettings.security, ...parsed.security },
      advanced: { ...defaultSettings.advanced, ...parsed.advanced },
      featurePacks: { ...defaultSettings.featurePacks, ...parsed.featurePacks },
      categories,
    };
  } catch (err) {
    console.error("Failed to parse settings, falling back to defaults", err);
    return defaultSettings;
  }
}

export function saveSettings(settings: EdmSettings): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
    // Also sync the theme and palette for CSS hooks
    document.documentElement.setAttribute("data-mode", settings.general.theme === "system" ? "dark" : settings.general.theme);
    document.documentElement.setAttribute("data-palette", settings.general.palette);
    document.documentElement.setAttribute("data-theme", settings.general.theme === "system" ? "dark" : settings.general.theme);
    // Sync to native backend store
    if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
      import("../services/tauriBridge").then(({ tauriSaveSettings }) => {
        tauriSaveSettings(settings).catch(() => {});
      }).catch(() => {});
    }
  } catch (err) {
    console.error("Failed to save settings", err);
  }
}

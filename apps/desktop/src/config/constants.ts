import type { CategoryRule } from "../types/settings";

export const APP_NAME = "EDM";
export const APP_SUBTITLE = "Evo Download Manager";
import pkg from "../../package.json";

export const APP_VERSION = pkg.version;

export const DOWNLOADS_STORAGE_KEY = "edm_downloads_v1";
export const DISMISSED_UPDATE_VERSION_KEY = "edm_dismissed_update_version";

export const DEFAULT_MAX_CONNECTIONS = 32;
export const DEFAULT_FALLBACK_SIZE_BYTES = 100_000_000;
export const BACKEND_SYNC_INTERVAL_MS = 800;
export const TELEMETRY_TICK_INTERVAL_MS = 250;
export const UI_THROTTLE_MS = 500;
export const STALLED_THRESHOLD_MS = 10_000;
export const LOCAL_PREVIEW_GRACE_MS = 5_000;

export const SPEED_LIMIT_BYTES_MAP: Record<string, number | null> = {
  Unlimited: null,
  "50 MB/s": 50 * 1024 * 1024,
  "20 MB/s": 20 * 1024 * 1024,
  "10 MB/s": 10 * 1024 * 1024,
  "5 MB/s": 5 * 1024 * 1024,
  "2 MB/s": 2 * 1024 * 1024,
  "500 KB/s": 500 * 1024,
};

export const VALID_DOWNLOAD_CATEGORIES = [
  "Video",
  "Music",
  "Documents",
  "Compressed",
  "Programs",
  "Pictures",
  "Other",
] as const;

export type DownloadCategory = (typeof VALID_DOWNLOAD_CATEGORIES)[number];
export type DownloadState = "downloading" | "paused" | "queued" | "completed" | "failed";

export const BACKEND_STATE_MAP: Record<string, DownloadState> = {
  downloading: "downloading",
  verifying: "downloading",
  paused: "paused",
  cancelled: "paused",
  completed: "completed",
  failed: "failed",
  queued: "queued",
  created: "queued",
  probing: "queued",
};

export const CATEGORY_EXTENSIONS: Record<Exclude<DownloadCategory, "Programs" | "Other">, readonly string[]> = {
  Video: ["mp4", "mkv", "webm", "avi", "mov", "flv", "wmv"],
  Music: ["mp3", "flac", "wav", "aac", "ogg", "m4a"],
  Compressed: ["zip", "rar", "7z", "tar", "gz", "iso", "bin"],
  Documents: ["pdf", "doc", "docx", "txt", "xlsx", "pptx"],
  Pictures: ["jpg", "jpeg", "png", "gif", "webp", "avif", "svg", "bmp", "ico", "tiff", "tif"],
};

export function inferCategoryFromExtension(
  ext: string,
  categories?: CategoryRule[]
): DownloadCategory {
  const normalized = ext.toLowerCase();
  for (const [category, extensions] of Object.entries(CATEGORY_EXTENSIONS)) {
    if (extensions.includes(normalized)) {
      return category as DownloadCategory;
    }
  }
  const matchedCat = categories?.find((c) => c.enabled && c.extensions.includes(normalized));
  return (matchedCat ? matchedCat.name : "Programs") as DownloadCategory;
}

export function ensureSingleEdmPath(base: string): string {
  if (!base) return "";
  const normalized = base.replace(/\//g, "\\");
  const stripped = normalized.replace(/(\\EDM)+$/i, "");
  return `${stripped}\\EDM`;
}

export function buildCategoryFolderMap(dirs: {
  music: string;
  videos: string;
  pictures: string;
  documents: string;
  downloads: string;
}): Record<string, string> {
  return {
    audio: ensureSingleEdmPath(dirs.music),
    music: ensureSingleEdmPath(dirs.music),
    video: ensureSingleEdmPath(dirs.videos),
    pictures: ensureSingleEdmPath(dirs.pictures),
    documents: ensureSingleEdmPath(dirs.documents),
    programs: `${dirs.downloads}\\Programs`,
    archives: `${dirs.downloads}\\Archives`,
  };
}

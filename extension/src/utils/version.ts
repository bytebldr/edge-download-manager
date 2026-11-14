/**
 * version.ts — Shared semver parsing and comparison utilities for EDM extension.
 */

export interface ParsedSemver {
  major: number;
  minor: number;
  patch: number;
}

export function parseSemver(v: string | null | undefined): ParsedSemver | null {
  if (!v || typeof v !== "string") return null;
  const clean = v.trim().replace(/^[vV]/, "");
  const core = clean.split(/[-+]/)[0].trim();
  const parts = core.split(".");
  if (parts.length < 3) return null;

  const major = parseInt(parts[0], 10);
  const minor = parseInt(parts[1], 10);
  const patch = parseInt(parts[2], 10);

  if (Number.isNaN(major) || Number.isNaN(minor) || Number.isNaN(patch)) {
    return null;
  }

  return { major, minor, patch };
}

/**
 * Compare two versions semantically.
 * Returns:
 *   -1 if a < b
 *    0 if a === b
 *    1 if a > b
 */
export function compareVersions(a: string | null | undefined, b: string | null | undefined): -1 | 0 | 1 {
  const va = parseSemver(a);
  const vb = parseSemver(b);

  if (va && vb) {
    if (va.major !== vb.major) return va.major > vb.major ? 1 : -1;
    if (va.minor !== vb.minor) return va.minor > vb.minor ? 1 : -1;
    if (va.patch !== vb.patch) return va.patch > vb.patch ? 1 : -1;
    return 0;
  }

  const sa = (a || "").trim().replace(/^[vV]/, "");
  const sb = (b || "").trim().replace(/^[vV]/, "");
  if (sa === sb) return 0;
  return sa > sb ? 1 : -1;
}

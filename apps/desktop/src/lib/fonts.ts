export type FontFamilyId =
  | "system"
  | "poppins"
  | "open-sans"
  | "space-grotesk"
  | "quicksand"
  | "ubuntu"
  | "urbanist";

export interface FontOption {
  id: FontFamilyId;
  label: string;
  stack: string;
  previewText: string;
}

export const SYSTEM_FONT_STACK =
  '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif';

export const FONT_OPTIONS: FontOption[] = [
  {
    id: "system",
    label: "System Default",
    stack: SYSTEM_FONT_STACK,
    previewText: "Native OS",
  },
  {
    id: "poppins",
    label: "Poppins",
    stack: `"Poppins", ${SYSTEM_FONT_STACK}`,
    previewText: "Geometric Sans",
  },
  {
    id: "open-sans",
    label: "Open Sans",
    stack: `"Open Sans", ${SYSTEM_FONT_STACK}`,
    previewText: "Humanist Sans",
  },
  {
    id: "space-grotesk",
    label: "Space Grotesk",
    stack: `"Space Grotesk", ${SYSTEM_FONT_STACK}`,
    previewText: "Technical Display",
  },
  {
    id: "quicksand",
    label: "Quicksand",
    stack: `"Quicksand", ${SYSTEM_FONT_STACK}`,
    previewText: "Rounded Sans",
  },
  {
    id: "ubuntu",
    label: "Ubuntu",
    stack: `"Ubuntu", ${SYSTEM_FONT_STACK}`,
    previewText: "Linux Classic",
  },
  {
    id: "urbanist",
    label: "Urbanist",
    stack: `"Urbanist", ${SYSTEM_FONT_STACK}`,
    previewText: "Modern Clean",
  },
];

export function getFontStack(id?: string): string {
  const match = FONT_OPTIONS.find((f) => f.id === id);
  return match ? match.stack : SYSTEM_FONT_STACK;
}

export function applyFontFamily(id?: string): void {
  if (typeof document === "undefined") return;
  const stack = getFontStack(id);
  document.documentElement.style.setProperty("--font-ui", stack);
  document.documentElement.style.setProperty("--font-sans", stack);
}

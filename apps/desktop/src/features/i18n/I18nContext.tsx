import React, { createContext, useCallback, useContext, useEffect, useMemo, useState } from "react";
import en from "./locales/en.json";
import es from "./locales/es.json";
import hi from "./locales/hi.json";
import ar from "./locales/ar.json";
import zh from "./locales/zh.json";

export type Locale = "en" | "es" | "hi" | "ar" | "zh";
export type Direction = "ltr" | "rtl";

export interface LocaleOption {
  code: Locale;
  label: string;
  nativeLabel: string;
  dir: Direction;
}

export interface I18nContextValue {
  locale: Locale;
  dir: Direction;
  setLocale: (locale: Locale) => void;
  t: (key: string, vars?: Record<string, string | number>) => string;
  availableLocales: readonly LocaleOption[];
}

type TranslationDict = Record<string, unknown>;

const STORAGE_KEY = "edm.locale";

const BUNDLES: Record<Locale, TranslationDict> = { en, es, hi, ar, zh };

export const AVAILABLE_LOCALES: readonly LocaleOption[] = [
  { code: "en", label: "English", nativeLabel: "English", dir: "ltr" },
  { code: "es", label: "Spanish", nativeLabel: "Español", dir: "ltr" },
  { code: "hi", label: "Hindi", nativeLabel: "हिन्दी", dir: "ltr" },
  { code: "ar", label: "Arabic", nativeLabel: "العربية", dir: "rtl" },
  { code: "zh", label: "Chinese", nativeLabel: "中文", dir: "ltr" },
];

function isLocale(value: unknown): value is Locale {
  return typeof value === "string" && value in BUNDLES;
}

function readInitialLocale(defaultLocale: Locale): Locale {
  try {
    const stored = window.localStorage.getItem(STORAGE_KEY);
    if (isLocale(stored)) return stored;
  } catch {
    // Ignore storage access errors in restricted environments.
  }
  return defaultLocale;
}

function lookup(bundle: TranslationDict, key: string): string | undefined {
  let cur: unknown = bundle;
  for (const part of key.split(".")) {
    if (cur == null || typeof cur !== "object") return undefined;
    cur = (cur as Record<string, unknown>)[part];
  }
  return typeof cur === "string" ? cur : undefined;
}

function formatTemplate(template: string, vars?: Record<string, string | number>): string {
  if (!vars) return template;
  return template.replace(/\{(\w+)\}/g, (_, k: string) =>
    k in vars ? String(vars[k]) : `{${k}}`
  );
}

const I18nContext = createContext<I18nContextValue | null>(null);

export interface I18nProviderProps {
  children: React.ReactNode;
  defaultLocale?: Locale;
}

/** Provides active locale, text direction, and translation lookup across the desktop UI. */
export function I18nProvider({ children, defaultLocale = "en" }: I18nProviderProps) {
  const [locale, setLocaleState] = useState<Locale>(() => readInitialLocale(defaultLocale));

  const dir: Direction = locale === "ar" ? "rtl" : "ltr";

  useEffect(() => {
    document.documentElement.dir = dir;
    document.documentElement.lang = locale;
  }, [dir, locale]);

  const setLocale = useCallback((next: Locale) => {
    setLocaleState(next);
    try {
      window.localStorage.setItem(STORAGE_KEY, next);
    } catch {
      // Ignore storage access errors in restricted environments.
    }
  }, []);

  const t = useCallback(
    (key: string, vars?: Record<string, string | number>): string => {
      const value = lookup(BUNDLES[locale], key) ?? lookup(BUNDLES.en, key) ?? key;
      return formatTemplate(value, vars);
    },
    [locale]
  );

  const value = useMemo<I18nContextValue>(
    () => ({
      locale,
      dir,
      setLocale,
      t,
      availableLocales: AVAILABLE_LOCALES,
    }),
    [locale, dir, setLocale, t]
  );

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

/** Returns current i18n state and translation helper; falls back to English outside a provider. */
export function useI18n(): I18nContextValue {
  const ctx = useContext(I18nContext);
  if (ctx) return ctx;
  return {
    locale: "en",
    dir: "ltr",
    setLocale: () => {},
    t: (key: string, vars?: Record<string, string | number>) =>
      formatTemplate(lookup(BUNDLES.en, key) ?? key, vars),
    availableLocales: AVAILABLE_LOCALES,
  };
}

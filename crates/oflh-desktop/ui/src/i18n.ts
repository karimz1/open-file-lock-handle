import { useSyncExternalStore } from "react";
import zh from "./locales/zh";
import de from "./locales/de";
import en, { type MessageKey } from "./locales/en";

const LANGUAGES = {
  en: { catalog: en, label: "language.k_english" },
  de: { catalog: de, label: "language.k_german" },
  zh: { catalog: zh, label: "language.k_chinese_simplified" },
} as const;

export type { MessageKey };
export type Locale = keyof typeof LANGUAGES;
export type LocalePreference = "system" | Locale;

export const languageOptions: {
  value: LocalePreference;
  label: MessageKey;
}[] = [
  { value: "system", label: "language.k_system_default" },
  ...Object.entries(LANGUAGES).map(([value, language]) => ({
    value: value as Locale,
    label: language.label,
  })),
];

const localeStorageKey = "oflh-language";
const localeListeners = new Set<() => void>();

function flattenMessages(
  messages: Record<string, unknown>,
  prefix = "",
): [string, string][] {
  return Object.entries(messages).flatMap(([key, value]) => {
    const path = prefix ? `${prefix}.${key}` : key;
    return typeof value === "string"
      ? [[value, path]]
      : flattenMessages(value as Record<string, unknown>, path);
  });
}
function messageAt(catalog: Record<string, unknown>, path: string): string {
  const value = path.split(".").reduce<unknown>((current, key) => {
    if (typeof current !== "object" || current === null) return undefined;
    return (current as Record<string, unknown>)[key];
  }, catalog);
  if (typeof value !== "string")
    throw new Error(`Missing translation: ${path}`);
  return value;
}

const sourceKeys = new Map<string, MessageKey>(
  flattenMessages(en).map(([message, key]) => [message, key as MessageKey]),
);

function systemLanguage(): string | undefined {
  return typeof navigator === "undefined" ? undefined : navigator.language;
}

export function resolveLocale(language: string | null | undefined): Locale {
  const primaryLanguage = language?.trim().split(/[-_]/, 1)[0].toLowerCase();
  return primaryLanguage && Object.hasOwn(LANGUAGES, primaryLanguage)
    ? (primaryLanguage as Locale)
    : "en";
}

export function resolvePreferredLocale(
  preference: LocalePreference,
  systemLanguage: string | null | undefined,
): Locale {
  return preference === "system" ? resolveLocale(systemLanguage) : preference;
}

export function isLocalePreference(
  value: string | null | undefined,
): value is LocalePreference {
  return value === "system" || (!!value && Object.hasOwn(LANGUAGES, value));
}

export function readLocalePreference(): LocalePreference {
  try {
    const saved = localStorage.getItem(localeStorageKey);
    if (isLocalePreference(saved)) return saved;
  } catch {
    // Locale selection remains available for the current system language.
  }
  return "system";
}

let localePreferenceState = readLocalePreference();
let localeState = resolvePreferredLocale(
  localePreferenceState,
  systemLanguage(),
);

function emitLocaleChange() {
  for (const listener of localeListeners) listener();
}

export function getLocalePreference(): LocalePreference {
  return localePreferenceState;
}

export function getLocale(): Locale {
  return localeState;
}

export function subscribeLocale(onStoreChange: () => void): () => void {
  localeListeners.add(onStoreChange);
  return () => {
    localeListeners.delete(onStoreChange);
  };
}

export function applyDocumentLang(language: Locale = localeState): void {
  if (typeof document === "undefined") return;
  document.documentElement.lang = language;
}

export function writeLocalePreference(preference: LocalePreference): void {
  try {
    localStorage.setItem(localeStorageKey, preference);
  } catch {
    // Apply the language for this session even without storage.
  }
}

export function changeLocalePreference(
  preference: LocalePreference,
  language: string | null | undefined = systemLanguage(),
): Locale {
  localePreferenceState = preference;
  localeState = resolvePreferredLocale(preference, language);
  writeLocalePreference(preference);
  applyDocumentLang(localeState);
  emitLocaleChange();
  return localeState;
}

export function useLocalePreference(): LocalePreference {
  return useSyncExternalStore(
    subscribeLocale,
    getLocalePreference,
    getLocalePreference,
  );
}

export function useLocale(): Locale {
  return useSyncExternalStore(subscribeLocale, getLocale, getLocale);
}

export function translate(
  language: Locale,
  key: MessageKey,
  values: Record<string, string | number> = {},
): string {
  return messageAt(LANGUAGES[language].catalog, key).replace(
    /\{\{(\w+)\}\}/g,
    (placeholder, name: string) =>
      values[name] === undefined ? placeholder : String(values[name]),
  );
}

export function t(
  key: MessageKey,
  values?: Record<string, string | number>,
): string {
  return translate(localeState, key, values);
}

export function tValue(value: string): string {
  const key = sourceKeys.get(value);
  return key ? t(key) : value;
}

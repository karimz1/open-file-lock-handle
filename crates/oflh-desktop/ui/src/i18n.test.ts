import { describe, expect, it } from "vitest";
import {
  resolveLocale,
  resolvePreferredLocale,
  translate,
  tValue,
} from "./i18n";
import de from "./locales/de";
import en from "./locales/en";
import zh from "./locales/zh";

function flattenCatalog(
  catalog: Record<string, unknown>,
  prefix = "",
): Record<string, string> {
  return Object.fromEntries(
    Object.entries(catalog).flatMap(([key, value]) => {
      const path = prefix ? `${prefix}.${key}` : key;
      if (typeof value === "string") return [[path, value]];
      return Object.entries(
        flattenCatalog(value as Record<string, unknown>, path),
      );
    }),
  );
}

describe("desktop locale selection", () => {
  it("requires complete locale coverage and matching interpolation placeholders", () => {
    const english = flattenCatalog(en);
    for (const catalog of [de, zh]) {
      const translated = flattenCatalog(catalog);
      expect(Object.keys(translated).sort()).toEqual(
        Object.keys(english).sort(),
      );

      for (const [key, englishMessage] of Object.entries(english)) {
        const translatedMessage = translated[key];
        expect(translatedMessage.trim(), key).not.toBe("");
        expect(
          [...translatedMessage.matchAll(/\{\{(\w+)\}\}/g)]
            .map((match) => match[1])
            .sort(),
        ).toEqual(
          [...englishMessage.matchAll(/\{\{(\w+)\}\}/g)]
            .map((match) => match[1])
            .sort(),
        );
      }
    }
  });

  it.each(["de", "de-DE", "de-AT", "de_CH"])(
    "selects German for %s",
    (language) => {
      expect(resolveLocale(language)).toBe("de");
    },
  );

  it.each(["zh", "zh-CN", "zh-SG", "zh-Hans", "zh_Hans_CN", "ZH-cn"])(
    "selects Simplified Chinese for %s",
    (language) => {
      expect(resolveLocale(language)).toBe("zh");
    },
  );

  it("translates Chinese labels and preserves interpolated values", () => {
    expect(translate("zh", "navigation.k_settings")).toBe("设置");
    expect(
      translate("zh", "table.k_resize_column_column", { column: "路径" }),
    ).toBe("调整路径列宽");
    expect(
      translate(
        "zh",
        "search.k_search_shortcut_f_or_escape_returns_to_e2053cec",
        {
          shortcut: "Ctrl",
        },
      ),
    ).toBe("搜索（Ctrl+F 或 /）；按 Escape 返回工作区");
  });

  it.each(["en", "en-US", "pt-PT", "fr-FR", "", undefined, null])(
    "falls back to English for %s",
    (language) => {
      expect(resolveLocale(language)).toBe("en");
    },
  );

  it("translates known German messages and preserves English fallback text", () => {
    expect(translate("de", "navigation.k_settings")).toBe("Einstellungen");
    expect(translate("en", "navigation.k_settings")).toBe("Settings");
    expect(
      translate("de", "table.k_resize_column_column", { column: "Pfad" }),
    ).toBe("Größe der Spalte Pfad ändern");
    expect(tValue("An untranslated diagnostic")).toBe(
      "An untranslated diagnostic",
    );
  });

  it("honors an explicit language and follows system language when requested", () => {
    expect(resolvePreferredLocale("zh", "en-US")).toBe("zh");
    expect(resolvePreferredLocale("system", "zh-CN")).toBe("zh");
    expect(resolvePreferredLocale("de", "pt-PT")).toBe("de");
    expect(resolvePreferredLocale("en", "de-DE")).toBe("en");
    expect(resolvePreferredLocale("system", "de-DE")).toBe("de");
    expect(resolvePreferredLocale("system", "pt-PT")).toBe("en");
  });
});

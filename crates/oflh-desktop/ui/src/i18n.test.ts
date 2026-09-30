import { describe, expect, it } from "vitest";
import { resolveLocale, translate } from "./i18n";

describe("desktop locale selection", () => {
  it.each(["de", "de-DE", "de-AT", "de_CH"])(
    "selects German for %s",
    (language) => {
      expect(resolveLocale(language)).toBe("de");
    },
  );

  it.each(["en", "en-US", "pt-PT", "fr-FR", "", undefined, null])(
    "falls back to English for %s",
    (language) => {
      expect(resolveLocale(language)).toBe("en");
    },
  );

  it("translates known German messages and preserves English fallback text", () => {
    expect(translate("de", "Settings")).toBe("Einstellungen");
    expect(translate("en", "Settings")).toBe("Settings");
    expect(translate("de", "An untranslated diagnostic")).toBe(
      "An untranslated diagnostic",
    );
  });
});

import { useEffect, useState } from "react";
import { Check, Monitor } from "lucide-react";
import { t, type MessageKey } from "./i18n";
type ThemeOption = {
  id: string;
  label: MessageKey;
  description: MessageKey;
  colors: readonly string[];
};
export const themes = [
  {
    id: "system",
    label: "themes.k_system",
    description: "themes.k_follow_your_device",
    colors: ["#f2f4f7", "#242424", "#a3a3a3"],
  },
  {
    id: "light",
    label: "themes.k_light",
    description: "themes.k_bright_and_clear",
    colors: ["#ffffff", "#f2f4f7", "#4264db"],
  },
  {
    id: "rider",
    label: "themes.k_rider_dark",
    description: "themes.k_rider_inspired_charcoal_with_crisp_contrast",
    colors: ["#17191e", "#2a2f38", "#86c2ff"],
  },
  {
    id: "vscode",
    label: "themes.k_vs_code_dark",
    description: "themes.k_graphite_with_blue_accents",
    colors: ["#1e1e1e", "#30343c", "#75beff"],
  },
  {
    id: "purple",
    label: "themes.k_oflh_purple",
    description: "themes.k_colors_from_the_terminal_demo",
    colors: ["#20202b", "#282536", "#b79aff"],
  },
] as const satisfies readonly ThemeOption[];
export type Theme = (typeof themes)[number]["id"];
export type AppliedTheme = Exclude<Theme, "system">;

/** Keep the saved System preference separate from the concrete design it uses. */
export function useAppliedTheme(theme: Theme): AppliedTheme {
  const [systemDark, setSystemDark] = useState(
    () => matchMedia("(prefers-color-scheme: dark)").matches,
  );
  useEffect(() => {
    const media = matchMedia("(prefers-color-scheme: dark)");
    const update = () => setSystemDark(media.matches);
    update();
    media.addEventListener("change", update);
    return () => media.removeEventListener("change", update);
  }, []);
  return theme === "system" ? (systemDark ? "vscode" : "light") : theme;
}

export function themeLabel(theme: Theme): string {
  const option = themes.find((option) => option.id === theme);
  return option ? t(option.label) : theme;
}

export function readTheme(): { theme: Theme; firstUse: boolean } {
  try {
    const saved = localStorage.getItem("oflh-theme");
    if (saved === "dark") return { theme: "vscode", firstUse: false };
    if (themes.some((item) => item.id === saved))
      return { theme: saved as Theme, firstUse: false };
  } catch {
    /* Preferences may be unavailable; retain a usable session default. */
  }
  return { theme: "vscode", firstUse: true };
}
export function ThemePicker({
  theme,
  appliedTheme,
  onChange,
}: {
  theme: Theme;
  appliedTheme: AppliedTheme;
  onChange: (theme: Theme) => void;
}) {
  return (
    <div className="theme-options">
      {themes.map(({ id, label, description, colors }) => (
        <button
          key={id}
          aria-label={t(label)}
          aria-pressed={
            id === "system" ? theme === "system" : appliedTheme === id
          }
          className={
            appliedTheme === id ? "theme-choice active" : "theme-choice"
          }
          onClick={() =>
            onChange(id === "system" && theme === "system" ? appliedTheme : id)
          }
        >
          <span
            className="theme-preview"
            aria-hidden="true"
            style={{
              background: colors[0],
              borderColor: colors[1],
            }}
          >
            <span style={{ background: colors[1] }} />
            <i style={{ background: colors[2] }} />
          </span>
          <span className="theme-caption">
            <strong>{t(label)}</strong>
            <small>
              {id === "system" && theme === "system"
                ? t("themes.k_system_uses_theme", {
                    theme: themeLabel(appliedTheme),
                  })
                : t(description)}
            </small>
            {theme === "system" && id === appliedTheme && (
              <small className="system-theme-note">
                {t("themes.k_applied_by_system")}
              </small>
            )}
          </span>
          {appliedTheme === id && <Check size={16} aria-hidden="true" />}
          {theme === "system" && id === "system" && (
            <Monitor size={16} aria-hidden="true" />
          )}
        </button>
      ))}
    </div>
  );
}

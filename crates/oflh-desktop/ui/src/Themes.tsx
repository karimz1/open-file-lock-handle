import { Check } from "lucide-react";
export const themes = [
  {
    id: "system",
    label: "System",
    description: "Follow your device",
    colors: ["#f2f4f7", "#242424", "#a3a3a3"],
  },
  {
    id: "light",
    label: "Light",
    description: "Bright and clear",
    colors: ["#ffffff", "#f2f4f7", "#4264db"],
  },
  {
    id: "rider",
    label: "Rider Dark",
    description: "Inspired by Rider",
    colors: ["#191a1c", "#27282c", "#a9b9d0"],
  },
  {
    id: "vscode",
    label: "VS Code Dark",
    description: "Graphite with blue accents",
    colors: ["#1e1e1e", "#252526", "#75beff"],
  },
  {
    id: "purple",
    label: "OFLH Purple",
    description: "Colors from the terminal demo",
    colors: ["#20202b", "#282536", "#b79aff"],
  },
] as const;
export type Theme = (typeof themes)[number]["id"];

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
  onChange,
}: {
  theme: Theme;
  onChange: (theme: Theme) => void;
}) {
  return (
    <div className="theme-options">
      {themes.map(({ id, label, description, colors }) => (
        <button
          key={id}
          aria-label={label}
          aria-pressed={theme === id}
          className={theme === id ? "theme-choice active" : "theme-choice"}
          onClick={() => onChange(id)}
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
            <strong>{label}</strong>
            <small>{description}</small>
          </span>
          {theme === id && <Check size={16} aria-hidden="true" />}
        </button>
      ))}
    </div>
  );
}

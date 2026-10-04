import { useEffect, useRef, useState } from "react";
import {
  Check,
  ChevronRight,
  Download,
  LoaderCircle,
  Palette,
  RefreshCw,
  Settings,
} from "lucide-react";
import { t } from "./i18n";
import { themes, type Theme } from "./Themes";
import type { Updates } from "./useUpdates";

export function SettingsMenu({
  updates,
  active,
  openSettings,
  theme,
  onThemeChange,
}: {
  updates: Updates;
  active: boolean;
  openSettings: () => void;
  theme: Theme;
  onThemeChange: (theme: Theme) => void;
}) {
  const [open, setOpen] = useState(false);
  const [themesOpen, setThemesOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const themeTrigger = useRef<HTMLButtonElement>(null);
  const themeMenu = useRef<HTMLDivElement>(null);
  const focusTheme = useRef(false);
  const { state } = updates;
  const available =
    state.phase === "available" || state.phase === "install-failed";
  const busy = updates.checking || state.phase === "installing";
  const close = () => {
    setOpen(false);
    setThemesOpen(false);
    trigger.current?.focus();
  };
  const openThemes = () => {
    // Hover may already have mounted the submenu; focus it immediately then.
    focusTheme.current = !themeMenu.current;
    setThemesOpen(true);
    themeMenu.current
      ?.querySelector<HTMLButtonElement>('[aria-checked="true"]')
      ?.focus();
  };
  useEffect(() => {
    if (themesOpen && focusTheme.current) {
      themeMenu.current
        ?.querySelector<HTMLButtonElement>('[aria-checked="true"]')
        ?.focus();
      focusTheme.current = false;
    }
  }, [themesOpen]);
  useEffect(() => {
    if (!open) {
      setThemesOpen(false);
      return;
    }
    root.current
      ?.querySelector<HTMLButtonElement>('[role="menuitem"]')
      ?.focus();
    const outside = (event: PointerEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("pointerdown", outside);
    return () => document.removeEventListener("pointerdown", outside);
  }, [open]);
  return (
    <div
      className="settings-menu"
      ref={root}
      onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget as Node | null))
          setOpen(false);
      }}
      onKeyDown={(event) => {
        if (!open) return;
        const inThemes = themeMenu.current?.contains(document.activeElement);
        if (
          (event.key === "Escape" && themesOpen) ||
          (event.key === "ArrowLeft" && inThemes)
        ) {
          event.preventDefault();
          event.stopPropagation();
          setThemesOpen(false);
          themeTrigger.current?.focus();
          return;
        }
        if (event.key === "Escape") {
          event.preventDefault();
          event.stopPropagation();
          close();
          return;
        }
        if (
          event.key === "ArrowRight" &&
          document.activeElement === themeTrigger.current
        ) {
          event.preventDefault();
          event.stopPropagation();
          openThemes();
          return;
        }
        if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key))
          return;
        event.preventDefault();
        event.stopPropagation();
        const menu =
          document.activeElement?.closest('[role="menu"]') ??
          root.current?.querySelector('[role="menu"]');
        const items = [
          ...(menu?.querySelectorAll<HTMLButtonElement>(
            ":scope > button:not(:disabled), :scope > .theme-menu-group > button",
          ) ?? []),
        ];
        const index = items.indexOf(
          document.activeElement as HTMLButtonElement,
        );
        const next =
          event.key === "Home"
            ? 0
            : event.key === "End"
              ? items.length - 1
              : (index + (event.key === "ArrowDown" ? 1 : -1) + items.length) %
                items.length;
        if (!inThemes && items[next] !== themeTrigger.current)
          setThemesOpen(false);
        items[next]?.focus();
      }}
    >
      <button
        ref={trigger}
        title={t("navigation.k_settings")}
        className={active ? "active settings-trigger" : "settings-trigger"}
        aria-label={t("navigation.k_settings")}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? "settings-menu" : undefined}
        aria-describedby={available ? "update-notification" : undefined}
        onClick={() => setOpen(!open)}
      >
        <span className="settings-icon">
          <Settings size={20} />
          {available && (
            <span className="update-badge" aria-hidden="true">
              1
            </span>
          )}
        </span>
      </button>
      {available && (
        <span id="update-notification" className="sr-only" role="status">
          {t("update.k_notification")}
        </span>
      )}
      {open && (
        <div
          id="settings-menu"
          className="settings-popup"
          role="menu"
          aria-label={t("navigation.k_settings")}
        >
          <button
            role="menuitem"
            onMouseEnter={() => setThemesOpen(false)}
            onClick={() => {
              close();
              openSettings();
            }}
          >
            <Settings size={15} />
            {t("navigation.k_settings")}
          </button>
          <div
            className="theme-menu-group"
            onMouseEnter={() => setThemesOpen(true)}
            onMouseLeave={() => {
              if (!themeMenu.current?.contains(document.activeElement))
                setThemesOpen(false);
            }}
          >
            <button
              ref={themeTrigger}
              role="menuitem"
              aria-haspopup="menu"
              aria-expanded={themesOpen}
              aria-controls={themesOpen ? "theme-menu" : undefined}
              onClick={openThemes}
            >
              <Palette size={15} />
              {t("themes.k_themes")}
              <ChevronRight size={15} className="menu-chevron" />
            </button>
            {themesOpen && (
              <div
                id="theme-menu"
                ref={themeMenu}
                className="settings-popup themes-submenu"
                role="menu"
                aria-label={t("themes.k_themes")}
              >
                {themes.map(({ id, label, colors }) => (
                  <button
                    key={id}
                    role="menuitemradio"
                    aria-checked={theme === id}
                    onClick={() => {
                      onThemeChange(id);
                      close();
                    }}
                  >
                    <span
                      className="menu-theme-swatch"
                      aria-hidden="true"
                      style={{ background: colors[0], borderColor: colors[2] }}
                    />
                    {t(label)}
                    {theme === id && (
                      <Check
                        size={15}
                        className="menu-chevron"
                        aria-hidden="true"
                      />
                    )}
                  </button>
                ))}
              </div>
            )}
          </div>
          <div className="settings-menu-rule" role="separator" />
          <button
            role="menuitem"
            disabled={busy || state.phase === "restart-required"}
            onMouseEnter={() => setThemesOpen(false)}
            onClick={() => {
              close();
              if (available) updates.requestUpdate();
              else void updates.runCheck();
            }}
          >
            {busy ? (
              <LoaderCircle size={15} className="spin" />
            ) : available ? (
              <Download size={15} />
            ) : (
              <RefreshCw size={15} />
            )}
            {available
              ? t("update.k_new_update_available")
              : t(
                  state.phase === "installing"
                    ? "update.k_installing_update"
                    : state.phase === "checking"
                      ? "update.k_checking_for_updates"
                      : state.phase === "restart-required"
                        ? "update.k_update_installed_restart_oflh_to_finish"
                        : "update.k_check_for_updates",
                )}
            {available && (
              <span className="menu-update-count" aria-hidden="true">
                1
              </span>
            )}
          </button>
        </div>
      )}
    </div>
  );
}

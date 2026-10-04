import { useEffect, useRef, useState } from "react";
import { Download, LoaderCircle, RefreshCw, Settings } from "lucide-react";
import { t } from "./i18n";
import type { Updates } from "./useUpdates";

export function SettingsMenu({
  updates,
  active,
  openSettings,
}: {
  updates: Updates;
  active: boolean;
  openSettings: () => void;
}) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const { state } = updates;
  const available =
    state.phase === "available" || state.phase === "install-failed";
  const busy = state.phase === "checking" || state.phase === "installing";
  const close = () => {
    setOpen(false);
    trigger.current?.focus();
  };
  useEffect(() => {
    if (!open) return;
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
        if (event.key === "Escape") {
          event.stopPropagation();
          close();
        }
        if (
          !open ||
          !["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)
        )
          return;
        event.preventDefault();
        event.stopPropagation();
        const items = [
          ...root.current!.querySelectorAll<HTMLButtonElement>(
            '[role="menuitem"]:not(:disabled)',
          ),
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
          <Settings size={17} />
          {available && (
            <span className="update-badge" aria-hidden="true">
              1
            </span>
          )}
        </span>
        <span className="nav-label">{t("navigation.k_settings")}</span>
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
            onClick={() => {
              close();
              openSettings();
            }}
          >
            <Settings size={15} />
            {t("navigation.k_settings")}
          </button>
          <div className="settings-menu-rule" />
          <button
            role="menuitem"
            disabled={busy || state.phase === "restart-required"}
            onClick={() => {
              close();
              void (available ? updates.activate() : updates.runCheck());
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
              ? t(
                  state.update.kind === "download"
                    ? "update.k_download_update"
                    : "update.k_install_and_restart",
                )
              : t(
                  state.phase === "installing"
                    ? "update.k_installing_update"
                    : state.phase === "checking"
                      ? "update.k_checking_for_updates"
                      : state.phase === "restart-required"
                        ? "update.k_update_installed_restart_oflh_to_finish"
                        : "update.k_check_for_updates",
                )}
            {available && <span className="menu-update-count">1</span>}
          </button>
        </div>
      )}
    </div>
  );
}

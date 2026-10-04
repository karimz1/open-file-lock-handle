import { Download, LoaderCircle, RefreshCw, ShieldAlert } from "lucide-react";
import { api } from "./api";
import { t } from "./i18n";

import type { Updates } from "./useUpdates";

export function UpdateBanner({ state, runCheck, activate }: Updates) {
  const viewReleaseNotes = () => void api.openReleaseNotes().catch(() => {});
  return (
    <section className="setting-section updates">
      <div>
        <h3>
          <RefreshCw size={17} />
          {t("update.k_updates")}
        </h3>
        {state.phase === "checking" ? (
          <p className="muted">
            <LoaderCircle size={14} className="spin" />{" "}
            {t("update.k_checking_for_updates")}
          </p>
        ) : state.phase === "current" ? (
          <p className="muted">{t("update.k_you_re_up_to_date")}</p>
        ) : state.phase === "available" ? (
          <p>
            {t("update.k_update_available_version", {
              version: state.update.version,
            })}{" "}
            {t("update.k_a_newer_version_is_available")}
          </p>
        ) : state.phase === "installing" ? (
          <p className="muted">
            <LoaderCircle size={14} className="spin" />{" "}
            {t("update.k_installing_update")}
          </p>
        ) : state.phase === "restart-required" ? (
          <p>{t("update.k_update_installed_restart_oflh_to_finish")}</p>
        ) : (
          <p className="muted">
            <ShieldAlert size={14} />{" "}
            {t(
              state.phase === "install-failed"
                ? "update.k_update_action_failed"
                : "update.k_update_check_failed",
            )}
          </p>
        )}
      </div>
      <div className="inline-actions">
        {(state.phase === "available" || state.phase === "install-failed") && (
          <button onClick={() => void activate()}>
            <Download size={14} />{" "}
            {t(
              state.update.kind === "download"
                ? "update.k_download_update"
                : "update.k_install_and_restart",
            )}
          </button>
        )}
        {state.phase !== "checking" &&
          state.phase !== "installing" &&
          state.phase !== "restart-required" && (
            <button onClick={() => void runCheck()}>
              <RefreshCw size={14} /> {t("update.k_check_for_updates")}
            </button>
          )}
        <button onClick={viewReleaseNotes}>
          {t("update.k_view_release_notes")}
        </button>
      </div>
    </section>
  );
}

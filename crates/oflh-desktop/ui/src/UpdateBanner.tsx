import { useCallback, useEffect, useState } from "react";
import { Download, LoaderCircle, RefreshCw, ShieldAlert } from "lucide-react";
import { checkForUpdate, installUpdate, type Update } from "./updater";
import { api } from "./api";
import { t } from "./i18n";

type UpdateState =
  | { phase: "idle" }
  | { phase: "checking" }
  | { phase: "current" }
  | { phase: "available"; update: Update }
  | { phase: "installing" }
  | { phase: "restart-required" }
  | { phase: "unsupported" }
  | { phase: "failed" };

/** Settings-page section that checks the configured updater endpoint and
 * offers a silent install, falling back to the releases page when the
 * platform package (for example Linux .deb/.rpm) cannot self-update. */
export function UpdateBanner() {
  const [state, setState] = useState<UpdateState>({ phase: "idle" });

  const runCheck = useCallback(async () => {
    setState({ phase: "checking" });
    try {
      const update = await checkForUpdate();
      setState(update ? { phase: "available", update } : { phase: "current" });
    } catch {
      // The updater plugin is unavailable on some packages (for example a
      // Linux .deb/.rpm install); direct users to the releases page instead.
      setState({ phase: "unsupported" });
    }
  }, []);

  useEffect(() => {
    void runCheck();
  }, [runCheck]);

  const install = useCallback(async () => {
    if (state.phase !== "available") return;
    setState({ phase: "installing" });
    try {
      await installUpdate(state.update);
      setState({ phase: "restart-required" });
    } catch {
      setState({ phase: "failed" });
    }
  }, [state]);

  const viewReleaseNotes = () => void api.openReleaseNotes().catch(() => {});

  return (
    <section className="setting-section updates">
      <div>
        <h3>
          <RefreshCw size={17} />
          {t("update.k_updates")}
        </h3>
        {state.phase === "idle" || state.phase === "checking" ? (
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
        ) : state.phase === "unsupported" ? (
          <p className="muted">
            {t("update.k_update_not_supported_on_this_package")}
          </p>
        ) : (
          <p className="muted">
            <ShieldAlert size={14} /> {t("update.k_update_check_failed")}
          </p>
        )}
      </div>
      <div className="inline-actions">
        {state.phase === "available" && (
          <button onClick={() => void install()}>
            <Download size={14} /> {t("update.k_install_and_restart")}
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

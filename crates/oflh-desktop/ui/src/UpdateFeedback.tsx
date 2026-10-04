import { useEffect } from "react";
import {
  Check,
  Download,
  ExternalLink,
  LoaderCircle,
  ShieldAlert,
  X,
} from "lucide-react";
import { api } from "./api";
import { t } from "./i18n";
import { Modal } from "./Modal";
import type { Updates } from "./useUpdates";

/** Manual checks report their result without interrupting quiet startup checks. */
export function UpdateFeedback({
  updates,
  report,
}: {
  updates: Updates;
  report: (failure: unknown) => void;
}) {
  const { state, notice, clearNotice } = updates;
  useEffect(() => {
    if (!notice) return;
    const timer = setTimeout(clearNotice, 5000);
    return () => clearTimeout(timer);
  }, [notice, clearNotice]);
  const hasUpdate =
    state.phase === "available" ||
    state.phase === "installing" ||
    state.phase === "install-failed";
  const installing = state.phase === "installing";
  return (
    <>
      {notice && (
        <div className="toast update-toast" role="status">
          {notice === "current" ? (
            <Check size={16} />
          ) : (
            <ShieldAlert size={16} />
          )}
          {t(
            notice === "current"
              ? "update.k_you_re_up_to_date"
              : "update.k_update_check_failed",
          )}
          <button
            className="icon-button"
            aria-label={t("common.k_close")}
            onClick={clearNotice}
          >
            <X size={14} />
          </button>
        </div>
      )}
      {updates.dialogOpen && hasUpdate && (
        <Modal
          title={t("update.k_new_update_available")}
          close={updates.closeDialog}
        >
          <div className="update-dialog-version">
            <Download size={20} />
            <strong>
              {t("update.k_update_available_version", {
                version: state.update.version,
              })}
            </strong>
          </div>
          <p>{t("update.k_update_recommendation")}</p>
          <p className="muted">
            {t(
              state.update.kind === "install"
                ? "update.k_install_question"
                : "update.k_manual_download",
            )}
          </p>
          <button
            className="inline-link update-release-notes"
            onClick={() => void api.openReleaseNotes().catch(report)}
          >
            {t("update.k_view_release_notes")} <ExternalLink size={14} />
          </button>
          {state.phase === "install-failed" && (
            <p className="danger-text update-dialog-status" role="alert">
              {t("update.k_update_action_failed")}
            </p>
          )}
          {installing && (
            <p className="update-dialog-status" role="status">
              <LoaderCircle size={15} className="spin" />{" "}
              {t("update.k_installing_update")}
            </p>
          )}
          <div className="modal-actions">
            <button data-default-focus onClick={updates.closeDialog}>
              {t("update.k_later")}
            </button>
            <button
              className="primary"
              disabled={installing}
              onClick={() => void updates.activate()}
            >
              {installing ? (
                <LoaderCircle size={15} className="spin" />
              ) : state.update.kind === "install" ? (
                <Download size={15} />
              ) : (
                <ExternalLink size={15} />
              )}
              {t(
                installing
                  ? "update.k_installing_update"
                  : state.update.kind === "install"
                    ? "update.k_install_and_restart"
                    : "update.k_go_to_download_page",
              )}
            </button>
          </div>
        </Modal>
      )}
    </>
  );
}

import { useEffect, useState } from "react";
import { FileX2, Network, ShieldAlert } from "lucide-react";
import { api, type CloseFileConfirmation, type ShareFiles } from "./api";
import { Modal } from "./Modal";
import { availabilityHint } from "./sessions";
import { t } from "./i18n";

/** Files of the current target opened by other people over SMB (Windows only). */
export function SharePanel({
  revision,
  notify,
  report,
}: {
  revision: number;
  notify: (message: string) => void;
  report: (failure: unknown) => void;
}) {
  const [shares, setShares] = useState<ShareFiles | null>(null);
  const [confirm, setConfirm] = useState<CloseFileConfirmation | null>(null);
  const [reload, setReload] = useState(0);
  useEffect(() => {
    let live = true;
    api
      .shareFiles()
      .then((value) => live && setShares(value))
      .catch(() => live && setShares(null));
    return () => {
      live = false;
    };
  }, [revision, reload]);
  if (!shares || shares.close === "unsupported") return null;
  if (!shares.files.length && !shares.error) return null;
  const blocked = availabilityHint(shares.close);
  const count = shares.files.length;
  return (
    <details className="share-panel">
      <summary>
        <Network size={13} aria-hidden="true" />
        {t("sessions.k_share_title")}
        {count > 0 && <span className="badge">{count}</span>}
        <span className="muted share-summary">
          {shares.error
            ? t("sessions.k_share_unreadable")
            : count === 1
              ? t("sessions.k_share_summary_one")
              : t("sessions.k_share_summary_many", { n: count })}
        </span>
      </summary>
      <ul className="share-list">
        {shares.files.map((file) => (
          <li key={file.id}>
            <span className="impact-primary" title={file.path}>
              {file.path.split(/[\\/]/).pop() || file.path}
            </span>
            <span className="impact-secondary">
              {[
                file.user,
                file.client,
                file.write ? t("sessions.k_share_write") : null,
                file.locks
                  ? t("sessions.k_share_locks", { n: file.locks })
                  : null,
              ]
                .filter(Boolean)
                .join(" · ")}
            </span>
            <button
              type="button"
              disabled={blocked !== null}
              title={blocked?.hint}
              onClick={() =>
                void api
                  .prepareCloseFile(file.id)
                  .then(setConfirm)
                  .catch(report)
              }
            >
              <FileX2 size={13} aria-hidden="true" />
              {t("sessions.k_share_close")}
            </button>
          </li>
        ))}
        {blocked && (
          <li className="session-requirement share-requirement">
            <ShieldAlert size={12} aria-hidden="true" />
            {blocked.label}. {blocked.hint}
          </li>
        )}
      </ul>
      {confirm && (
        <Modal
          danger
          className="confirm-modal"
          title={t("sessions.k_share_close_title", {
            path: confirm.file.path.split(/[\\/]/).pop() || confirm.file.path,
            user: confirm.file.user,
          })}
          close={() => {
            setConfirm(null);
            void api.dismiss();
          }}
        >
          <p className="danger-note">
            {t("sessions.k_share_close_note", { user: confirm.file.user })}
          </p>
          <p className="mono detail-path">{confirm.file.path}</p>
          <div className="modal-actions">
            <button
              data-default-focus
              onClick={() => {
                setConfirm(null);
                void api.dismiss();
              }}
            >
              {t("common.k_cancel")}
            </button>
            <button
              className="danger"
              onClick={() => {
                const pending = confirm;
                setConfirm(null);
                api
                  .closeFile(pending.ticket)
                  .then(() =>
                    notify(
                      t("sessions.k_share_closed", { user: pending.file.user }),
                    ),
                  )
                  .catch(report)
                  .finally(() => setReload((value) => value + 1));
              }}
            >
              {t("sessions.k_share_close")}
            </button>
          </div>
        </Modal>
      )}
    </details>
  );
}

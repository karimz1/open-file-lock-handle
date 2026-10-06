import { useEffect, useRef, useState } from "react";
import { LoaderCircle } from "lucide-react";
import { api, errorMessage, type Status } from "./api";
import { acceptStatus } from "./state";
import { t, type MessageKey } from "./i18n";

const phaseMessages: Record<string, MessageKey> = {
  processes: "inspection.k_checking_process_references",
  files: "inspection.k_checking_files",
  ports: "inspection.k_checking_ports",
  indexing: "inspection.k_preparing_results",
};
export function InspectionOverlay({
  status,
  starting,
  complete,
}: {
  status: Status;
  starting: boolean;
  complete: (status: Status) => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [live, setLive] = useState(status);
  const [error, setError] = useState("");
  const [cancelling, setCancelling] = useState(false);
  useEffect(() => {
    const element = dialog.current!;
    const previous = document.activeElement as HTMLElement | null;
    element.showModal();
    element.focus();
    return () => {
      element.close();
      if (previous?.isConnected) previous.focus();
    };
  }, []);
  useEffect(() => {
    setLive(status);
    if (!status.scanning) return;
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    // One request at a time, only while scanning. Progress updates are local to
    // this dialog so the grid does not rerender on every polling tick.
    const poll = async () => {
      try {
        const incoming = await api.status();
        if (!active) return;
        if (incoming.generation >= status.generation) {
          setLive((current) => acceptStatus(current, incoming));
          setError("");
          if (!incoming.scanning) {
            complete(incoming);
            return;
          }
        }
      } catch {
        if (active) setError(t("inspection.k_progress_unavailable"));
      }
      if (active) timer = setTimeout(poll, 250);
    };
    timer = setTimeout(poll, 250);
    return () => {
      active = false;
      clearTimeout(timer);
    };
  }, [status.generation, status.scanning, complete]);
  const cancel = async () => {
    if (starting || cancelling) return;
    setCancelling(true);
    try {
      complete(await api.cancel());
    } catch (failure) {
      setError(errorMessage(failure));
      setCancelling(false);
    }
  };
  return (
    <dialog
      ref={dialog}
      tabIndex={-1}
      className="modal inspection-dialog"
      aria-labelledby="inspection-title"
      aria-describedby="inspection-description"
      onCancel={(event) => event.preventDefault()}
      onKeyDown={(event) => {
        // F5 and app commands must neither restart the webview nor reach the
        // workspace handler while the modal inspection barrier is active.
        if (
          event.key === "F5" ||
          ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "r")
        )
          event.preventDefault();
        event.stopPropagation();
      }}
    >
      <div className="inspection-heading">
        <LoaderCircle size={24} className="spin" aria-hidden="true" />
        <div>
          <h2 id="inspection-title">{t("inspection.k_scanning")}</h2>
          <p id="inspection-description">{t("inspection.k_reload_paused")}</p>
        </div>
      </div>
      <div className="inspection-timing">
        <span>
          {t(
            starting && !status.scanning
              ? "inspection.k_starting_inspection"
              : (phaseMessages[live.progress?.phase] ??
                  "inspection.k_checking_process_references"),
          )}
        </span>
        <strong className="mono">
          {((live.elapsed_ms ?? 0) / 1000).toFixed(1)} s
        </strong>
      </div>
      <div
        className="inspection-progress"
        role="progressbar"
        aria-label={t("inspection.k_scanning")}
      />
      <div className="inspection-counts">
        {live.progress?.phase === "files" || !!live.progress?.files ? (
          <span>
            {t("inspection.k_files_checked", {
              count: live.progress?.files ?? 0,
            })}
          </span>
        ) : null}
        <span>
          {t("inspection.k_references_checked", {
            count: live.progress?.resources ?? 0,
          })}
        </span>
        <span>
          {t("inspection.k_processes_checked", {
            count: live.progress?.processes ?? 0,
          })}
        </span>
      </div>
      <p className="muted">{t("inspection.k_unknown_total")}</p>
      {error && <p role="alert">{error}</p>}
      <div className="modal-actions">
        <button disabled={starting || cancelling} onClick={() => void cancel()}>
          {t("common.k_cancel")}
        </button>
      </div>
    </dialog>
  );
}

import { useEffect, useRef, useState } from "react";
import {
  Check,
  ChevronDown,
  FolderOpen,
  Hourglass,
  Info,
  TimerOff,
  X,
} from "lucide-react";
import {
  api,
  errorMessage,
  type Status,
  type WaitSelection,
  type PathValue,
  type WaitFollowUpResult,
} from "./api";
import {
  validWaitName,
  waitPathParts,
  waitDestinationPreview,
} from "./waitActions";
import { Modal } from "./Modal";
import { locale, t } from "./i18n";

export type FollowUp = "none" | "reveal" | "delete" | "move" | "rename";
interface WaitOptions {
  selection: WaitSelection;
  timeoutSeconds: number | null;
  followUp: FollowUp;
  destination: string | null;
  newName: string | null;
  id?: number;
}
type FollowUpProgress =
  | { id: number; state: "running" }
  | { id: number; state: "done"; result: WaitFollowUpResult }
  | { id: number; state: "failed"; message: string };
export function elapsedLabel(ms: number) {
  const seconds = Math.floor(ms / 1000);
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}

function WaitDialog({
  selection,
  current,
  close,
  start,
  report,
}: {
  selection: WaitSelection;
  current: boolean;
  close: () => void;
  start: (options: WaitOptions) => Promise<void>;
  report: (error: unknown) => void;
}) {
  const [followUp, setFollowUp] = useState<FollowUp>("none");
  const [deadline, setDeadline] = useState(false);
  const [seconds, setSeconds] = useState(300);
  const [confirmed, setConfirmed] = useState(false);
  const [busy, setBusy] = useState(false);
  const [destination, setDestination] = useState<PathValue | null>(null);
  const [newName, setNewName] = useState("");
  const kind = selection.target.kind;
  const destructive =
    followUp === "delete" || followUp === "move" || followUp === "rename";
  const validName =
    validWaitName(newName) && newName !== waitPathParts(selection.display).name;
  const validAction =
    (followUp !== "move" || destination !== null) &&
    (followUp !== "rename" || validName);
  const resultPreview =
    followUp === "move" && destination
      ? waitDestinationPreview(destination.display, selection.display)
      : followUp === "rename" && validName
        ? waitPathParts(selection.display).parent + newName
        : null;
  const chooseDestination = async () => {
    setBusy(true);
    try {
      const folder = await api.chooseWaitDestination();
      if (folder) {
        setDestination(folder);
        setConfirmed(false);
      }
    } catch (error) {
      report(error);
    } finally {
      setBusy(false);
    }
  };
  const validTimeout =
    !deadline ||
    (Number.isInteger(seconds) && seconds >= 1 && seconds <= 86400);
  const begin = async () => {
    if (
      !current ||
      busy ||
      !validTimeout ||
      !validAction ||
      (destructive && !confirmed)
    )
      return;
    setBusy(true);
    try {
      await start({
        selection,
        timeoutSeconds: deadline ? seconds : null,
        followUp,
        destination: followUp === "move" ? destination!.reference : null,
        newName: followUp === "rename" ? newName : null,
      });
      close();
    } catch (error) {
      report(error);
    } finally {
      setBusy(false);
    }
  };
  return (
    <Modal
      className="wait-dialog"
      title={t(
        kind === "process"
          ? "wait.k_process_action"
          : kind === "port"
            ? "wait.k_port_action"
            : "wait.k_file_action",
      )}
      close={() => {
        if (!busy) close();
      }}
    >
      <div className="wait-target-card">
        <span className="eyebrow">
          {t(
            kind === "process"
              ? "wait.k_process_target"
              : kind === "port"
                ? "wait.k_port_target"
                : "wait.k_file_target",
          )}
        </span>
        <p className={kind === "process" ? "" : "mono"}>{selection.display}</p>
      </div>
      <p className="hint wait-description">
        <Info size={16} aria-hidden="true" />
        {t(
          kind === "process"
            ? "wait.k_process_description"
            : kind === "port"
              ? "wait.k_port_description"
              : "wait.k_file_description",
        )}
      </p>
      <div className="wait-fields">
        <label className="checkbox-label">
          <input
            type="checkbox"
            disabled={busy}
            checked={deadline}
            onChange={(event) => setDeadline(event.target.checked)}
          />
          {t("wait.k_enable_timeout")}
        </label>
        {deadline && (
          <label className="wait-field">
            {t("wait.k_seconds")}
            <input
              type="number"
              disabled={busy}
              min="1"
              max="86400"
              step="1"
              value={Number.isFinite(seconds) ? seconds : ""}
              onChange={(event) =>
                setSeconds(
                  event.target.value === "" ? NaN : Number(event.target.value),
                )
              }
            />
          </label>
        )}
        {kind === "file" && (
          <label className="wait-field">
            {t("wait.k_follow_up")}
            <span className="wait-select">
              <select
                value={followUp}
                disabled={busy}
                onChange={(event) => {
                  setFollowUp(event.target.value as FollowUp);
                  setConfirmed(false);
                }}
              >
                <option value="none">{t("wait.k_notify_only")}</option>
                <option value="reveal">{t("wait.k_reveal")}</option>
                <option value="move">{t("wait.k_move")}</option>
                <option value="rename">{t("wait.k_rename")}</option>
                <option value="delete">{t("wait.k_delete")}</option>
              </select>
              <ChevronDown aria-hidden="true" />
            </span>
          </label>
        )}
        {followUp === "move" && (
          <div className="wait-destination">
            <span className="wait-field-label">{t("wait.k_destination")}</span>
            <div className="wait-destination-picker">
              <p className={destination ? "mono" : "muted"}>
                {destination?.display || t("wait.k_no_destination")}
              </p>
              <button
                type="button"
                disabled={busy}
                onClick={() => void chooseDestination()}
              >
                <FolderOpen size={15} />
                {t(
                  destination
                    ? "wait.k_change_destination"
                    : "wait.k_choose_destination",
                )}
              </button>
            </div>
          </div>
        )}
        {followUp === "rename" && (
          <label className="wait-field">
            {t("wait.k_new_name")}
            <input
              type="text"
              aria-label={t("wait.k_new_name")}
              disabled={busy}
              value={newName}
              spellCheck={false}
              placeholder={waitPathParts(selection.display).name}
              onChange={(event) => {
                setNewName(event.target.value);
                setConfirmed(false);
              }}
              aria-invalid={newName !== "" && !validName}
              aria-describedby="wait-name-hint"
            />
            <span
              id="wait-name-hint"
              className={newName !== "" && !validName ? "danger-text" : "hint"}
            >
              {t(
                newName !== "" && !validName
                  ? "wait.k_invalid_name"
                  : "wait.k_name_hint",
              )}
            </span>
          </label>
        )}
        {resultPreview && (
          <div className="wait-result-preview">
            <span className="wait-field-label">{t("wait.k_result_path")}</span>
            <p className="mono">{resultPreview}</p>
            <p className="hint">{t("wait.k_reveal_after_change")}</p>
          </div>
        )}
        {destructive && (
          <label className="checkbox-label wait-confirm">
            <input
              type="checkbox"
              disabled={busy}
              checked={confirmed}
              onChange={(event) => setConfirmed(event.target.checked)}
            />
            {t(
              followUp === "delete"
                ? "wait.k_confirm_delete"
                : followUp === "rename"
                  ? "wait.k_confirm_rename"
                  : "wait.k_confirm_move",
            )}
          </label>
        )}
      </div>
      {!current && (
        <p className="hint" role="status">
          {t("wait.k_selection_changed")}
        </p>
      )}
      <div className="modal-actions">
        <button data-default-focus disabled={busy} onClick={close}>
          {t("common.k_cancel")}
        </button>
        <button
          className="primary"
          disabled={
            !current ||
            busy ||
            !validTimeout ||
            !validAction ||
            (destructive && !confirmed)
          }
          onClick={() => void begin()}
        >
          <Hourglass size={15} />
          {t("wait.k_start")}
        </button>
      </div>
    </Modal>
  );
}

/** Selection actions open the dialog; only an active or completed wait has a status banner. */
export function WaitUntilFree({
  status,
  selection,
  disabled,
  report,
  onStatus,
  onChoose,
  onClose,
}: {
  status: Status;
  selection: WaitSelection | null;
  disabled: boolean;
  report: (error: unknown) => void;
  onStatus: (status: Status) => void;
  onChoose: (selection: WaitSelection) => void;
  onClose: () => void;
}) {
  const [options, setOptions] = useState<WaitOptions | null>(null);
  const [elapsed, setElapsed] = useState(0);
  const [dismissed, setDismissed] = useState<number | null>(null);
  const [progress, setProgress] = useState<FollowUpProgress | null>(null);
  const liveWait = useRef(status.wait.id);
  liveWait.current = status.wait.id;
  const completed = useRef(false);
  const ready = status.wait.outcome === "free";
  const target =
    status.wait.target ||
    (options
      ? {
          kind: options.selection.target.kind,
          display: options.selection.display,
        }
      : null);
  useEffect(() => {
    if (!status.wait.waiting) return;
    const started = Date.now() - status.wait.elapsed_ms;
    setElapsed(status.wait.elapsed_ms);
    const timer = window.setInterval(
      () => setElapsed(Date.now() - started),
      1000,
    );
    return () => window.clearInterval(timer);
  }, [status.wait.waiting, status.wait.id]);
  useEffect(() => {
    if (
      !ready ||
      !options ||
      completed.current ||
      status.wait.id !== options.id ||
      options.id === undefined
    )
      return;
    completed.current = true;
    const id = options.id;
    if (options.followUp === "none") {
      void api.notifyWaitFree(id, locale).catch(report);
      return;
    }
    setProgress({ id, state: "running" });
    void api
      .waitFollowUp(
        id,
        options.followUp,
        options.destination,
        options.newName,
        locale,
      )
      .then((result) => {
        if (liveWait.current === id) setProgress({ id, state: "done", result });
      })
      .catch((error) => {
        if (liveWait.current === id) {
          setProgress({ id, state: "failed", message: errorMessage(error) });
          report(error);
        }
      });
  }, [ready, options, status.wait.id, report]);
  const begin = async (next: WaitOptions) => {
    const waiting = await api.waitForSelection(
      next.selection,
      next.timeoutSeconds,
    );
    completed.current = false;
    setDismissed(null);
    setProgress(null);
    setOptions({ ...next, id: waiting.wait.id });
    onStatus(waiting);
  };
  const failure =
    status.wait.error?.kind === "wait_timeout"
      ? t("wait.k_timeout")
      : status.wait.error?.kind === "wait_target_held"
        ? t("wait.k_held_again")
        : status.wait.error?.kind === "cancelled"
          ? t("wait.k_cancelled")
          : status.wait.error?.message;
  const visible =
    target &&
    (status.wait.waiting || ready || failure) &&
    (status.wait.id === undefined || dismissed !== status.wait.id);
  const activeProgress = progress?.id === status.wait.id ? progress : null;
  const result =
    activeProgress?.state === "done" ? activeProgress.result : null;
  const resultLabel =
    result &&
    t(
      result.action === "move"
        ? "wait.k_moved"
        : result.action === "rename"
          ? "wait.k_renamed"
          : result.action === "delete"
            ? "wait.k_deleted"
            : "wait.k_revealed",
    );
  const displayedPath = result?.path || target?.display;
  return (
    <>
      {visible && (
        <section
          className={`wait-panel ${ready && activeProgress?.state !== "failed" ? "free" : ""}`}
          role="region"
          aria-label={t("wait.k_activity")}
          data-wait-panel
        >
          {status.wait.waiting || activeProgress?.state === "running" ? (
            <Hourglass size={16} />
          ) : ready && activeProgress?.state !== "failed" ? (
            <Check size={16} />
          ) : (
            <TimerOff size={16} />
          )}
          <strong role="status">
            {activeProgress?.state === "running"
              ? t("wait.k_follow_up_running")
              : activeProgress?.state === "failed"
                ? t("wait.k_follow_up_failed")
                : resultLabel ||
                  (status.wait.waiting
                    ? t(
                        target.kind === "process"
                          ? "wait.k_waiting_process"
                          : "wait.k_waiting",
                      )
                    : ready
                      ? t(
                          target.kind === "process"
                            ? "wait.k_process_exited"
                            : "wait.k_free",
                        )
                      : failure)}
          </strong>
          <span className="wait-resource" title={displayedPath}>
            {displayedPath}
          </span>
          {activeProgress?.state === "failed" && (
            <span className="wait-completion-warning">
              {activeProgress.message}
            </span>
          )}
          {result?.reveal_error && (
            <span className="wait-completion-warning" role="status">
              {t("wait.k_reveal_failed")} {result.reveal_error.message}
            </span>
          )}
          {result?.notification_error && (
            <span className="wait-completion-warning" role="status">
              {t("wait.k_notification_failed")}{" "}
              {result.notification_error.message}
            </span>
          )}
          {status.wait.waiting && (
            <>
              <span className="mono">{elapsedLabel(elapsed)}</span>
              <button
                onClick={() =>
                  void api.cancelWait().then(onStatus).catch(report)
                }
              >
                {t("common.k_cancel")}
              </button>
            </>
          )}
          {!status.wait.waiting && (
            <>
              {options && (!result || result.action === "reveal") && (
                <button
                  disabled={
                    disabled ||
                    activeProgress?.state === "running" ||
                    options.selection.revision !== status.revision
                  }
                  onClick={() => onChoose(options.selection)}
                >
                  {t("wait.k_again")}
                </button>
              )}
              <button
                className="icon-button"
                aria-label={t("wait.k_dismiss")}
                onClick={() => {
                  setDismissed(status.wait.id ?? 0);
                  setOptions(null);
                }}
              >
                <X size={15} />
              </button>
            </>
          )}
        </section>
      )}
      {selection && (
        <WaitDialog
          key={JSON.stringify(selection)}
          selection={selection}
          current={
            !disabled &&
            !status.wait.waiting &&
            selection.revision === status.revision
          }
          close={onClose}
          start={begin}
          report={report}
        />
      )}
    </>
  );
}

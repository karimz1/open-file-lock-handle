import { useEffect, useRef, useState } from "react";
import { Hourglass, TimerOff, X } from "lucide-react";
import { api, type Status } from "./api";
import { Modal } from "./Modal";
import { t } from "./i18n";
import { locale } from "./i18n";
import { normalizeTypedPath } from "./targetPath";

export type FollowUp = "none" | "reveal" | "delete" | "move";
export interface WaitOptions {
  path: string | null;
  includePorts: boolean;
  port: number | null;
  timeoutSeconds: number | null;
  followUp: FollowUp;
  destination: string | null;
}
export function waitTarget(path: string, portQuery: string) {
  const match = /^(?:.*\b)?port:(\d{1,5})\s*$/i.exec(portQuery.trim());
  const port = match ? Number(match[1]) : null;
  return {
    path: match ? null : normalizeTypedPath(path),
    port: port && port > 0 && port <= 65535 ? port : null,
  };
}
export function elapsedLabel(ms: number) {
  const seconds = Math.floor(ms / 1000);
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}

export function WaitUntilFree({
  status,
  portQuery,
  disabled,
  report,
  onStatus,
}: {
  status: Status;
  portQuery: string;
  disabled: boolean;
  report: (error: unknown) => void;
  onStatus: (status: Status) => void;
}) {
  const [configuring, setConfiguring] = useState(false);
  const [followUp, setFollowUp] = useState<FollowUp>("none");
  const [timeoutSeconds, setTimeoutSeconds] = useState(0);
  const [options, setOptions] = useState<WaitOptions | null>(null);
  const [elapsed, setElapsed] = useState(status.wait.elapsed_ms);
  const completed = useRef(false);
  const target = waitTarget(status.target, portQuery);
  const ready = status.wait.outcome === "free";

  useEffect(() => {
    if (!status.wait.waiting) return;
    setElapsed(status.wait.elapsed_ms);
    const started = Date.now() - status.wait.elapsed_ms;
    const timer = window.setInterval(
      () => setElapsed(Date.now() - started),
      250,
    );
    return () => window.clearInterval(timer);
  }, [status.wait.waiting, status.wait.elapsed_ms]);
  useEffect(() => {
    if (!ready || !options || completed.current) return;
    completed.current = true;
    const request =
      options.followUp === "none"
        ? api.notifyWaitFree(locale)
        : api.waitFollowUp(options.followUp, options.destination, locale);
    void request.catch(report);
  }, [ready, options, report]);

  const begin = async () => {
    const selectedTarget = waitTarget(status.target, portQuery);
    if (!selectedTarget.path && !selectedTarget.port) return;
    let destination: string | null = null;
    try {
      if (followUp === "move") {
        destination = await api.chooseWaitDestination();
        if (!destination) return;
      }
      if (followUp === "delete" && !window.confirm(t("wait.k_confirm_delete")))
        return;
      const next: WaitOptions = {
        ...selectedTarget,
        includePorts: !selectedTarget.port && status.target !== "",
        port: selectedTarget.port,
        timeoutSeconds: timeoutSeconds || null,
        followUp,
        destination,
      };
      setOptions(next);
      completed.current = false;
      const waiting = await api.waitUntilFree(
        selectedTarget.path,
        !selectedTarget.port,
        selectedTarget.port,
        next.timeoutSeconds,
      );
      onStatus(waiting);
      setConfiguring(false);
      if (waiting.wait.error) report(waiting.wait.error);
    } catch (error) {
      report(error);
    }
  };
  const cancel = () =>
    void api
      .cancelWait()
      .then((current) => {
        onStatus(current);
        setOptions(null);
      })
      .catch(report);
  const dismiss = () => {
    completed.current = true;
    setOptions(null);
  };
  const failure =
    status.wait.error?.kind === "wait_timeout"
      ? t("wait.k_timeout")
      : status.wait.error?.kind === "wait_target_held"
        ? t("wait.k_held_again")
        : status.wait.error?.kind === "cancelled"
          ? t("wait.k_cancelled")
          : status.wait.error?.message;

  if (!status.target && !portQuery && !status.wait.waiting) return null;
  return (
    <section
      inert={false}
      className={`wait-panel ${status.wait.waiting ? "waiting" : ""} ${ready ? "free" : ""}`}
      aria-live="polite"
      data-wait-panel
    >
      {!status.wait.waiting && !ready && !failure && (
        <button
          type="button"
          disabled={disabled || (!target.path && !target.port)}
          onClick={() => setConfiguring(true)}
        >
          <Hourglass size={15} /> {t("wait.k_action")}
        </button>
      )}
      {status.wait.waiting && (
        <>
          <Hourglass size={16} className="spin" />
          <strong>{t("wait.k_waiting")}</strong>
          <span>{t("wait.k_elapsed", { time: elapsedLabel(elapsed) })}</span>
          <span className="muted">
            {options?.timeoutSeconds
              ? t("wait.k_timeout_limit", { seconds: options.timeoutSeconds })
              : t("wait.k_no_timeout")}
          </span>
          <button type="button" onClick={cancel}>
            <X size={14} /> {t("common.k_cancel")}
          </button>
        </>
      )}
      {ready && (
        <>
          <span role="status">✓ {t("wait.k_free")}</span>
          <button
            type="button"
            onClick={() => {
              dismiss();
              setConfiguring(true);
            }}
          >
            {t("wait.k_again")}
          </button>
        </>
      )}
      {failure && !status.wait.waiting && (
        <>
          <TimerOff size={15} />
          <span>{failure}</span>
          <button
            type="button"
            onClick={() => {
              dismiss();
              setConfiguring(true);
            }}
          >
            {t("wait.k_again")}
          </button>
        </>
      )}
      {configuring && (
        <Modal title={t("wait.k_title")} close={() => setConfiguring(false)}>
          <p>{t("wait.k_description")}</p>
          <label>
            {t("wait.k_timeout_label")}
            <input
              aria-label={t("wait.k_timeout_label")}
              type="number"
              min="0"
              max="86400"
              value={timeoutSeconds}
              onChange={(event) =>
                setTimeoutSeconds(
                  Math.min(86400, Math.max(0, Number(event.target.value))),
                )
              }
            />
          </label>
          <label>
            {t("wait.k_follow_up")}
            <select
              aria-label={t("wait.k_follow_up")}
              value={followUp}
              disabled={!target.path}
              onChange={(event) => setFollowUp(event.target.value as FollowUp)}
            >
              <option value="none">{t("wait.k_notify_only")}</option>
              <option value="reveal">{t("wait.k_reveal")}</option>
              <option value="move">{t("wait.k_move")}</option>
              <option value="delete">{t("wait.k_delete")}</option>
            </select>
          </label>
          <div className="modal-actions">
            <button type="button" onClick={() => setConfiguring(false)}>
              {t("common.k_cancel")}
            </button>
            <button
              type="button"
              className="primary"
              onClick={() => void begin()}
            >
              {t("wait.k_start")}
            </button>
          </div>
        </Modal>
      )}
    </section>
  );
}

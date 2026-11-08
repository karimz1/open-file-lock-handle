import { useState } from "react";
import { LogOut, MessageSquare, ShieldAlert, Users } from "lucide-react";
import {
  api,
  type Availability,
  type Details,
  type Row,
  type SignOutConfirmation,
} from "./api";
import { Modal } from "./Modal";
import { ImpactGroup } from "./TerminationImpact";
import { availabilityHint, ownerLine } from "./sessions";
import { t } from "./i18n";
import {
  sessionMessage,
  sessionMessageContext,
  sessionMessageLength,
  sessionMessageNote,
  sessionMessageResource,
} from "./sessionMessage";

/** A session action button; disabled with a visible reason when rights are missing. */
function ActionButton({
  availability,
  icon,
  label,
  disabled,
  onClick,
}: {
  availability: Availability;
  icon: React.ReactNode;
  label: string;
  disabled: boolean;
  onClick: () => void;
}) {
  if (availability === "unsupported") return null;
  const blocked = availabilityHint(availability);
  return (
    <div className="session-action">
      <button
        type="button"
        disabled={disabled || blocked !== null}
        aria-describedby={blocked ? `${label}-why` : undefined}
        title={blocked?.hint}
        onClick={onClick}
      >
        {icon}
        {label}
      </button>
      {blocked && (
        <span className="session-requirement" id={`${label}-why`}>
          <ShieldAlert size={12} aria-hidden="true" />
          {blocked.label}
          <span className="sr-only">. {blocked.hint}</span>
        </span>
      )}
    </div>
  );
}

export function SessionPanel({
  details,
  revision,
  current,
  row,
  rowCurrent,
  target,
  notify,
  report,
}: {
  details: Details;
  revision: number;
  current: boolean;
  row: Row | null;
  rowCurrent: boolean;
  target: string;
  notify: (message: string) => void;
  report: (failure: unknown) => void;
}) {
  const owner = details.owner ?? {
    user: details.process.user,
    service: false,
    session: null,
  };
  const actions = details.session_actions;
  const [message, setMessage] = useState<{
    note: string;
    context: string;
    revision: number;
    processKey: string;
  } | null>(null);
  const [signOut, setSignOut] = useState<SignOutConfirmation | null>(null);
  const [busy, setBusy] = useState(false);
  const user = owner.session?.user || owner.user;
  const context = sessionMessageContext(details, row, target);
  const text = message ? sessionMessage(message.context, message.note) : "";
  const length = sessionMessageLength(text);
  const messageCurrent =
    current &&
    (!row || rowCurrent) &&
    (!message ||
      (message.revision === revision &&
        message.processKey === details.process.process_key &&
        message.context === context));
  const hint = (availability: Availability | undefined) =>
    availability ? availabilityHint(availability) : null;
  const run = (work: Promise<void>, done: string) => {
    setBusy(true);
    work
      .then(() => notify(done))
      .catch(report)
      .finally(() => setBusy(false));
  };
  return (
    <section className="session-panel" aria-label={t("sessions.k_owner")}>
      <h4>{t("sessions.k_owner")}</h4>
      <p className="owner-line">
        <Users size={13} aria-hidden="true" /> {ownerLine(owner)}
      </p>
      {actions && (
        <div className="inline-actions session-actions">
          <ActionButton
            availability={actions.send_message}
            icon={<MessageSquare size={13} />}
            label={t("sessions.k_send_message")}
            disabled={!messageCurrent || busy}
            onClick={() =>
              setMessage({
                note: sessionMessageNote(details, row),
                context,
                revision,
                processKey: details.process.process_key,
              })
            }
          />
          <ActionButton
            availability={actions.sign_out}
            icon={<LogOut size={13} />}
            label={t("sessions.k_sign_out")}
            disabled={!current || busy}
            onClick={() => {
              setBusy(true);
              api
                .prepareSignOut(revision, details.process.process_key)
                .then(setSignOut)
                .catch(report)
                .finally(() => setBusy(false));
            }}
          />
        </div>
      )}
      {message !== null && !hint(actions?.send_message) && (
        <Modal
          className="session-message-modal"
          title={t("sessions.k_message_title", { user })}
          close={() => setMessage(null)}
        >
          <p className="owner-line">{ownerLine(owner)}</p>
          <div className="session-message-context detail-path">
            <strong>
              {t("sessions.k_message_process", {
                name: details.process.name || t("app.k_unnamed"),
                pid: details.process.pid,
              })}
            </strong>
            <p>{sessionMessageResource(details, row, target)}</p>
          </div>
          <label className="message-field">
            {t("sessions.k_message_label")}
            <textarea
              data-default-focus
              maxLength={1000}
              rows={3}
              value={message.note}
              placeholder={t("sessions.k_message_placeholder")}
              onChange={(event) =>
                setMessage({ ...message, note: event.target.value })
              }
            />
          </label>
          {owner.session?.own && (
            <p className="hint">{t("sessions.k_message_own_session")}</p>
          )}
          <p className="hint">{t("sessions.k_message_delivery")}</p>
          <details className="session-message-preview" open>
            <summary>{t("sessions.k_message_preview")}</summary>
            <pre>{text}</pre>
          </details>
          <p className={length > 1000 ? "danger-text" : "hint"} role="status">
            {t("sessions.k_message_length", { n: length })}
            {length > 1000 && ` ${t("sessions.k_message_too_long")}`}
          </p>
          {!messageCurrent && (
            <p className="hint" role="status">
              {t("sessions.k_message_stale")}
            </p>
          )}
          <div className="modal-actions">
            <button onClick={() => setMessage(null)}>
              {t("common.k_cancel")}
            </button>
            <button
              className="primary"
              disabled={
                !message.note.trim() || length > 1000 || busy || !messageCurrent
              }
              onClick={() => {
                setMessage(null);
                run(
                  api.sendMessage(message.revision, message.processKey, text),
                  t("sessions.k_message_sent", { user }),
                );
              }}
            >
              {t("sessions.k_message_send")}
            </button>
          </div>
        </Modal>
      )}
      {signOut && (
        <Modal
          danger
          className="confirm-modal"
          title={t("sessions.k_sign_out_title", { user: signOut.session.user })}
          close={() => {
            setSignOut(null);
            void api.dismiss();
          }}
        >
          <p className="danger-note sign-out-note">
            {t("sessions.k_sign_out_note", { user: signOut.session.user })}
          </p>
          <p className="owner-line">
            {ownerLine({
              user: signOut.session.user,
              service: false,
              session: signOut.session,
            })}
          </p>
          <ImpactGroup
            kind="children"
            title={`${t("sessions.k_sign_out_processes")} (${signOut.processes.items.length + signOut.processes.more})`}
            list={signOut.processes}
          />
          <div className="modal-actions">
            <button
              data-default-focus
              onClick={() => {
                setSignOut(null);
                void api.dismiss();
              }}
            >
              {t("common.k_cancel")}
            </button>
            <button
              className="danger"
              onClick={() => {
                const confirmation = signOut;
                setSignOut(null);
                run(
                  api.signOut(confirmation.ticket),
                  t("sessions.k_sign_out_done", {
                    user: confirmation.session.user,
                  }),
                );
              }}
            >
              {t("sessions.k_sign_out_button", { user: signOut.session.user })}
            </button>
          </div>
        </Modal>
      )}
    </section>
  );
}

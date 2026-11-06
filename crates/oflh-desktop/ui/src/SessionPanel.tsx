import { useState } from "react";
import { LogOut, MessageSquare, ShieldAlert, Users } from "lucide-react";
import {
  api,
  type Availability,
  type Details,
  type SignOutConfirmation,
} from "./api";
import { Modal } from "./Modal";
import { ImpactGroup } from "./TerminationImpact";
import { availabilityHint, ownerLine } from "./sessions";
import { t } from "./i18n";

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
  notify,
  report,
}: {
  details: Details;
  revision: number;
  current: boolean;
  notify: (message: string) => void;
  report: (failure: unknown) => void;
}) {
  const owner = details.owner ?? {
    user: details.process.user,
    service: false,
    session: null,
  };
  const actions = details.session_actions;
  const [message, setMessage] = useState<string | null>(null);
  const [signOut, setSignOut] = useState<SignOutConfirmation | null>(null);
  const [busy, setBusy] = useState(false);
  const user = owner.session?.user || owner.user;
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
            disabled={!current || busy}
            onClick={() => setMessage("")}
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
          title={t("sessions.k_message_title", { user })}
          close={() => setMessage(null)}
        >
          <label className="message-field">
            {t("sessions.k_message_label")}
            <textarea
              data-default-focus
              maxLength={1000}
              rows={4}
              value={message}
              placeholder={t("sessions.k_message_placeholder")}
              onChange={(event) => setMessage(event.target.value)}
            />
          </label>
          <div className="modal-actions">
            <button onClick={() => setMessage(null)}>
              {t("common.k_cancel")}
            </button>
            <button
              className="primary"
              disabled={!message.trim() || busy}
              onClick={() => {
                const text = message;
                setMessage(null);
                run(
                  api.sendMessage(revision, details.process.process_key, text),
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

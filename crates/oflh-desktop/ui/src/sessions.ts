import type { Availability, OwnerView, SessionView } from "./api";
import { t, type MessageKey } from "./i18n";

/** Clock time of a logon, in the user's locale; today shows only HH:MM. */
export function sessionTime(
  since: number | null,
  now = Date.now(),
  locale?: string,
): string | null {
  if (since === null || !Number.isFinite(since) || since <= 0) return null;
  const date = new Date(since * 1000);
  const today = new Date(now).toDateString() === date.toDateString();
  return date.toLocaleString(
    locale,
    today
      ? { hour: "2-digit", minute: "2-digit" }
      : {
          day: "numeric",
          month: "short",
          hour: "2-digit",
          minute: "2-digit",
        },
  );
}

/** "Session 3 (RDP)", or "ssh pts/2" style when a terminal is known. */
export function sessionName(session: SessionView): string {
  const kind = t(`sessions.k_kind_${session.kind}` as MessageKey);
  if ((session.kind === "ssh" || session.kind === "tty") && session.tty)
    return `${kind} ${session.tty}`;
  return `${t("sessions.k_session_n", { id: session.id })} (${kind})`;
}

/** Short column text such as "3 · RDP". */
export function sessionCell(
  id: string | null | undefined,
  kind: string | null | undefined,
): string {
  if (!id) return "—";
  return kind ? `${id} · ${t(`sessions.k_kind_${kind}` as MessageKey)}` : id;
}

/** One line: "max.muster · Session 3 (RDP) · active since 08:12 · from LAPTOP-7". */
export function ownerLine(
  owner: OwnerView,
  now = Date.now(),
  locale?: string,
): string {
  const parts: string[] = [owner.user || t("common.k_unavailable_2c9c1f79")];
  if (owner.service) parts.push(t("sessions.k_service_account"));
  const session = owner.session;
  if (session) {
    parts.push(sessionName(session));
    const state = t(`sessions.k_state_${session.state}` as MessageKey);
    const time = sessionTime(session.since, now, locale);
    parts.push(time ? `${state} ${t("sessions.k_since", { time })}` : state);
    if (session.client)
      parts.push(t("sessions.k_from", { client: session.client }));
    if (session.own) parts.push(t("sessions.k_this_session"));
  }
  return parts.join(" · ");
}

/** Label and explanation for a disabled action, or null when it can run. */
export function availabilityHint(
  availability: Availability,
): { label: string; hint: string } | null {
  if (availability === "requires_admin")
    return {
      label: t("sessions.k_requires_admin"),
      hint: t("sessions.k_requires_admin_hint"),
    };
  if (availability === "requires_root")
    return {
      label: t("sessions.k_requires_root"),
      hint: t("sessions.k_requires_root_hint"),
    };
  return null;
}

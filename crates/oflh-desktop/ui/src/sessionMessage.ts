import type { Details, Row } from "./api";
import { t } from "./i18n";
import { sessionName } from "./sessions";

function messageFile(details: Details, row: Row | null): string {
  return row &&
    !row.port &&
    (row.matching_files || row.path_ref !== details.executable.reference)
    ? row.path
    : "";
}

/** Human-readable resource line shared by the compose form and recipient message. */
export function sessionMessageResource(
  details: Details,
  row: Row | null,
  target: string,
): string {
  const file = messageFile(details, row);
  if (file) return t("sessions.k_message_file", { path: file });
  if (row?.port)
    return t("sessions.k_message_port", {
      endpoint: row.port.endpoint,
      protocol: row.port.protocol,
    });
  return target ? t("sessions.k_message_target", { path: target }) : "";
}

/** Suggest a request appropriate to the selected resource, without claiming a lock. */
export function sessionMessageNote(details: Details, row: Row | null): string {
  return t(
    row?.port
      ? "sessions.k_message_port_note"
      : messageFile(details, row)
        ? "sessions.k_message_default_note"
        : "sessions.k_message_application_note",
  );
}

/** Context included even when the sender replaces the suggested note. */
export function sessionMessageContext(
  details: Details,
  row: Row | null,
  target: string,
): string {
  const process = details.process;
  const owner = details.owner;
  const recipient = owner?.session
    ? `${owner.user} · ${sessionName(owner.session)}`
    : owner?.user || process.user;
  const lines = [
    t("sessions.k_message_recipient", { user: recipient }),
    t("sessions.k_message_process", {
      name: process.name || t("app.k_unnamed"),
      pid: process.pid,
    }),
  ];
  // A process summary's executable is never described as the file being requested.
  const file = messageFile(details, row);
  const resource = sessionMessageResource(details, row, target);
  if (file) {
    lines.push(resource, "", t("sessions.k_message_file_reason"));
  } else if (row?.port) {
    lines.push(resource, "", t("sessions.k_message_port_reason"));
  } else if (target) {
    lines.push(resource, "", t("sessions.k_message_target_reason"));
  }
  return lines.join("\n");
}

/** Exact native message body, bounded separately against the service's character limit. */
export function sessionMessage(context: string, note: string): string {
  return [
    t("sessions.k_message_subject"),
    note.trim(),
    context,
    t("sessions.k_message_no_action"),
  ].join("\n\n");
}

/** Match Rust's Unicode scalar-value count rather than UTF-16 code units. */
export function sessionMessageLength(text: string): number {
  return [...text].length;
}

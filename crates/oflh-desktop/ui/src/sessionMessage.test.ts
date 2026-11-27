import { describe, expect, it } from "vitest";
import type { Details, Row } from "./api";
import { t } from "./i18n";
import {
  sessionMessage,
  sessionMessageContext,
  sessionMessageLength,
  sessionMessageNote,
} from "./sessionMessage";

const process: Row = {
  key: "424242:10:0/exe",
  process_key: "424242:10:0",
  name: "FixturePdfEditor",
  pid: 424242,
  user: "alice",
  path: "/build/PdfEditor.exe",
  path_ref: "1:0:exe",
  relation: "process",
  access: "unknown",
  evidence: null,
  evidence_label: null,
  deleted: false,
  memory: null,
  cpu: null,
  usages: 3,
  actionable: true,
  port: null,
  wait: { waiting: false, elapsed_ms: 0, outcome: null, error: null },
};
const details: Details = {
  process,
  ports: 0,
  can_inspect_folder: false,
  executable: { display: process.path, reference: process.path_ref },
  cwd: { display: "/build", reference: "1:0:cwd" },
  ancestors: [],
};

describe("native session message context", () => {
  it("retains the selected Unicode file and process when the sender replaces the note", () => {
    const row = {
      ...process,
      path: "/build/Bestellbestätigung .pdf",
      path_ref: "1:0:2",
    };
    const text = sessionMessage(
      sessionMessageContext(details, row, "/build"),
      "hey",
    );
    expect(text).toContain("hey");
    expect(text).toContain(row.path);
    expect(text).toContain("FixturePdfEditor");
    expect(text).toContain("424242");
    expect(text).toContain("alice");
    expect(text).toContain(t("sessions.k_message_file_reason"));
    expect(text).toContain(t("sessions.k_message_no_action"));
    expect(text).not.toContain("PdfEditor.exe");
    expect(sessionMessageNote(details, row)).toBe(
      t("sessions.k_message_default_note"),
    );
  });
  it("describes the inspected target rather than mistaking an executable for the requested file", () => {
    const text = sessionMessageContext(details, process, "/build/documents");
    expect(text).toContain("/build/documents");
    expect(text).not.toContain("PdfEditor.exe");
    expect(text).toContain(t("sessions.k_message_target_reason"));
    expect(sessionMessageNote(details, process)).toBe(
      t("sessions.k_message_application_note"),
    );
  });
  it("describes a port without carrying file context into port requests", () => {
    const row: Row = {
      ...process,
      port: {
        protocol: "TCP",
        state: "LISTEN",
        address: "::",
        number: 8080,
        endpoint: "[::]:8080",
        reference: "1:0:port-0",
      },
    };
    const text = sessionMessageContext(details, row, "/build/documents");
    expect(text).toContain("[::]:8080");
    expect(text).toContain("TCP");
    expect(text).not.toContain("/build/documents");
    expect(text).not.toContain("PdfEditor.exe");
    expect(sessionMessageNote(details, row)).toBe(
      t("sessions.k_message_port_note"),
    );
  });
  it("measures the complete native body using the same Unicode count as Rust", () => {
    expect(sessionMessageLength("😀".repeat(1000))).toBe(1000);
    const context = sessionMessageContext(details, null, "/build");
    const overhead = sessionMessageLength(sessionMessage(context, ""));
    expect(
      sessionMessageLength(
        sessionMessage(context, "x".repeat(1000 - overhead)),
      ),
    ).toBe(1000);
    expect(
      sessionMessageLength(
        sessionMessage(context, "x".repeat(1001 - overhead)),
      ),
    ).toBe(1001);
  });
});

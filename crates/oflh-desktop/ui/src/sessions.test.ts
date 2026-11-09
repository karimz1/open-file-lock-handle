import { describe, expect, it } from "vitest";
import type { OwnerView, SessionView } from "./api";
import { translate } from "./i18n";
import {
  availabilityHint,
  ownerLine,
  sessionCell,
  sessionName,
  sessionTime,
} from "./sessions";

const rdp: SessionView = {
  id: "3",
  user: "max.muster",
  domain: "CORP",
  kind: "rdp",
  state: "active",
  client: "LAPTOP-7",
  tty: null,
  since: Date.UTC(2026, 9, 29, 7, 12) / 1000,
  own: false,
};
const now = Date.UTC(2026, 9, 29, 12, 0);

describe("session wording", () => {
  it("names RDP, SSH and console sessions plainly", () => {
    expect(sessionName(rdp)).toBe("Session 3 (RDP)");
    expect(sessionName({ ...rdp, kind: "ssh", id: "c2", tty: "pts/2" })).toBe(
      "SSH pts/2",
    );
    expect(sessionName({ ...rdp, kind: "console", id: "1" })).toBe(
      "Session 1 (Console)",
    );
    expect(sessionCell("3", "rdp")).toBe("3 · RDP");
    expect(sessionCell(null, null)).toBe("—");
    expect(sessionCell("c2", null)).toBe("c2");
  });

  it("builds one owner line with state, time, client and own session", () => {
    const owner: OwnerView = {
      user: "max.muster",
      service: false,
      session: rdp,
    };
    const line = ownerLine(owner, now, "en-GB");
    expect(line).toMatch(
      /^max\.muster · Session 3 \(RDP\) · active since \d\d:\d\d · from LAPTOP-7$/,
    );
    expect(
      ownerLine(
        { ...owner, session: { ...rdp, own: true, client: null } },
        now,
      ),
    ).toMatch(/your session$/);
    expect(ownerLine({ user: "SYSTEM", service: true, session: null })).toBe(
      "SYSTEM · Service account",
    );
    expect(ownerLine({ user: "karim", service: false, session: null })).toBe(
      "karim",
    );
    expect(
      ownerLine({
        ...owner,
        session: { ...rdp, since: null, state: "disconnected" },
      }),
    ).toContain("· disconnected · from");
  });

  it("formats logon time as a clock today and with a date otherwise", () => {
    expect(sessionTime(null)).toBeNull();
    expect(sessionTime(0)).toBeNull();
    expect(sessionTime(rdp.since, now, "en-GB")).toMatch(/^\d\d:\d\d$/);
    expect(sessionTime(rdp.since, now + 3 * 86400_000, "en-GB")).toMatch(/Oct/);
  });

  it("explains why an action is disabled, and never for available ones", () => {
    expect(availabilityHint("available")).toBeNull();
    expect(availabilityHint("unsupported")).toBeNull();
    expect(availabilityHint("requires_admin")?.label).toBe(
      "Requires administrator rights",
    );
    expect(availabilityHint("requires_root")?.label).toBe("Requires root");
    expect(availabilityHint("requires_admin")?.hint).toMatch(/never asks/);
  });

  it("keeps the sign-out warning plain in every language", () => {
    const user = { user: "max.muster" };
    expect(translate("en", "sessions.k_sign_out_note", user)).toBe(
      "Every program in this session closes. Unsaved work of max.muster will be lost.",
    );
    expect(translate("de", "sessions.k_sign_out_note", user)).toContain(
      "geht verloren",
    );
    expect(translate("de", "sessions.k_requires_admin_hint")).toMatch(
      /^Starte OFLH/,
    );
    expect(translate("zh", "sessions.k_sign_out_title", user)).toBe(
      "注销 max.muster？",
    );
    expect(translate("de", "sessions.k_requires_root")).toBe("Erfordert root");
  });
});

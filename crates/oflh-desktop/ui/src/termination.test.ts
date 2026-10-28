import { describe, expect, it } from "vitest";
import type { ActionResult, Confirmation, ConfirmationTarget } from "./api";
import { forceRecoveryTargets, impactSummary } from "./termination";

const confirmation: Confirmation = {
  ticket: "fixture",
  force: false,
  elevated: false,
  targets: [
    { key: "42:10:0", name: "fixture", pid: 42 },
    { key: "43:11:0", name: "other", pid: 43 },
  ],
};
const result = (
  outcome: ActionResult["outcome"],
  kind?: string,
): ActionResult => ({
  pid: 42,
  outcome,
  error: kind ? { kind, message: "fixture failure", os_code: null } : null,
});
describe("force recovery", () => {
  it("offers only unsuccessful normal targets with captured identities", () => {
    for (const failure of [
      result("still_running"),
      result("failed", "permission_denied"),
      result("failed", "io"),
    ]) {
      expect(
        forceRecoveryTargets(confirmation, [
          failure,
          { pid: 43, outcome: "exited", error: null },
        ]),
      ).toEqual([confirmation.targets[0]]);
    }
  });
  it("never escalates force results or uncertain, stale, protected and cancelled targets", () => {
    expect(
      forceRecoveryTargets({ ...confirmation, force: true }, [
        result("still_running"),
      ]),
    ).toEqual([]);
    expect(forceRecoveryTargets(null, [result("still_running")])).toEqual([]);
    for (const outcome of ["exited", "unverified"] as const) {
      expect(forceRecoveryTargets(confirmation, [result(outcome)])).toEqual([]);
    }
    for (const kind of [
      "identity_changed",
      "protected",
      "cancelled",
      "unavailable",
      "not_found",
      "invalid_request",
    ]) {
      expect(
        forceRecoveryTargets(confirmation, [result("failed", kind)]),
      ).toEqual([]);
    }
  });
});

describe("termination impact summary", () => {
  const list = (count: number) => ({
    items: Array.from({ length: Math.min(count, 5) }, (_, n) => `item ${n}`),
    more: Math.max(0, count - 5),
  });
  const target = (impact: Partial<ConfirmationTarget["impact"]>) =>
    ({
      key: "42:10:0",
      name: "blender",
      pid: 42,
      role: "holder",
      impact: {
        windows: null,
        children: null,
        files: list(0),
        ports: list(0),
        ...impact,
      },
    }) as ConfirmationTarget;

  it("stays silent for the expected single window, file or port", () => {
    expect(impactSummary(target({}))).toEqual([]);
    expect(impactSummary(target({ windows: list(1), files: list(1) }))).toEqual(
      [],
    );
    expect(impactSummary(target({ ports: list(1) }))).toEqual([]);
    expect(impactSummary({ key: "1:1:0", name: "legacy", pid: 1 })).toEqual([]);
  });

  it("warns only about extra windows and counts capped remainders", () => {
    expect(
      impactSummary(
        target({
          windows: list(7),
          children: list(1),
          files: list(3),
          ports: list(1),
        }),
      ),
    ).toEqual([
      { key: "termination.k_impact_closes_windows", count: 7, warn: true },
      { key: "termination.k_impact_child_one", count: 1, warn: false },
      { key: "termination.k_impact_files", count: 3, warn: false },
      { key: "termination.k_impact_port_one", count: 1, warn: false },
    ]);
    expect(impactSummary(target({ ports: list(2) }))).toEqual([
      { key: "termination.k_impact_port_other", count: 2, warn: false },
    ]);
  });
});

import { describe, expect, it } from "vitest";
import type { ActionResult, Confirmation } from "./api";
import { forceRecoveryTargets } from "./termination";

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

import type { ActionResult, Confirmation } from "./api";

/** Match results to captured birth identities; never recover by the current selection. */
export function forceRecoveryTargets(
  confirmation: Confirmation | null,
  results: ActionResult[],
): Confirmation["targets"] {
  if (!confirmation || confirmation.force || confirmation.elevated) return [];
  return confirmation.targets.filter((target) =>
    results.some(
      (result) =>
        result.pid === target.pid &&
        (result.outcome === "still_running" ||
          (result.outcome === "failed" &&
            ![
              "identity_changed",
              "protected",
              "cancelled",
              "unavailable",
              "not_found",
              "invalid_request",
            ].includes(result.error?.kind ?? ""))),
    ),
  );
}

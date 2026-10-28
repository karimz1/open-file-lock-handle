import type {
  ActionResult,
  CappedList,
  Confirmation,
  ConfirmationTarget,
} from "./api";

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

/** One phrase of the collapsed impact line; `warn` marks the surprising ones. */
export interface ImpactPart {
  key:
    | "termination.k_impact_closes_windows"
    | "termination.k_impact_child_one"
    | "termination.k_impact_child_other"
    | "termination.k_impact_files"
    | "termination.k_impact_port_one"
    | "termination.k_impact_port_other";
  count: number;
  warn: boolean;
}

export const listTotal = (list: CappedList | null | undefined) =>
  list ? list.items.length + list.more : 0;

/**
 * Summarize only what goes beyond the selected row. One window, one scanned
 * file or the single port being inspected are expected and stay silent.
 */
export function impactSummary(target: ConfirmationTarget): ImpactPart[] {
  const impact = target.impact;
  if (!impact) return [];
  const parts: ImpactPart[] = [];
  const windows = listTotal(impact.windows);
  const children = listTotal(impact.children);
  const files = listTotal(impact.files);
  const ports = listTotal(impact.ports);
  if (windows > 1)
    parts.push({
      key: "termination.k_impact_closes_windows",
      count: windows,
      warn: true,
    });
  if (children > 0)
    parts.push({
      key:
        children === 1
          ? "termination.k_impact_child_one"
          : "termination.k_impact_child_other",
      count: children,
      warn: false,
    });
  if (files > 1)
    parts.push({
      key: "termination.k_impact_files",
      count: files,
      warn: false,
    });
  if (ports > 1 || (ports > 0 && files > 0))
    parts.push({
      key:
        ports === 1
          ? "termination.k_impact_port_one"
          : "termination.k_impact_port_other",
      count: ports,
      warn: false,
    });
  return parts;
}

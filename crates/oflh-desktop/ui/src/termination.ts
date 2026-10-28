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

/** One plain-language consequence; `warn` marks the surprising ones. */
export interface ImpactPart {
  key:
    | "termination.k_impact_windows"
    | "termination.k_impact_children_one"
    | "termination.k_impact_children_other"
    | "termination.k_impact_files_one"
    | "termination.k_impact_files_other"
    | "termination.k_impact_ports_one"
    | "termination.k_impact_ports_other";
  values: Record<string, string | number>;
  warn: boolean;
}

/** Collapsed rows stay at about two lines; everything else is in Details. */
export const IMPACT_SENTENCES = 2;

export const listTotal = (list: CappedList | null | undefined) =>
  list ? list.items.length + list.more : 0;

/**
 * Consequences beyond the selected row, most surprising first. One window,
 * one scanned file or the single port being inspected are expected and stay
 * silent.
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
      key: "termination.k_impact_windows",
      values: {
        name: target.name,
        count: windows,
        title: impact.windows!.items[0],
      },
      warn: true,
    });
  if (children > 0)
    parts.push({
      key:
        children === 1
          ? "termination.k_impact_children_one"
          : "termination.k_impact_children_other",
      values: { count: children },
      warn: false,
    });
  if (files > 1)
    parts.push({
      key:
        files === 2
          ? "termination.k_impact_files_one"
          : "termination.k_impact_files_other",
      values: { count: files - 1 },
      warn: false,
    });
  if (ports > 1 || (ports > 0 && files > 0))
    parts.push({
      key:
        ports === 1
          ? "termination.k_impact_ports_one"
          : "termination.k_impact_ports_other",
      values: { count: ports },
      warn: false,
    });
  return parts;
}

/** Split a scan label into what people recognize and the technical rest. */
export function impactRow(
  kind: "windows" | "children" | "files" | "ports",
  item: string,
): { primary: string; secondary: string; copy?: string } {
  if (kind === "children") {
    const match = /^(.*) \(PID (\d+)\)$/.exec(item);
    if (match)
      return {
        primary: match[1],
        secondary: `PID ${match[2]}`,
        copy: match[2],
      };
  }
  if (kind === "files") {
    const cut = Math.max(item.lastIndexOf("/"), item.lastIndexOf("\\"));
    if (cut > 0 && cut < item.length - 1)
      return {
        primary: item.slice(cut + 1),
        secondary: item.slice(0, cut),
        copy: item,
      };
    return { primary: item, secondary: "", copy: item };
  }
  if (kind === "ports") {
    const match = /^(\S+) (?:(.*):)?(\d+)$/.exec(item);
    if (match)
      return {
        primary: match[3],
        secondary: match[2] ? `${match[1]} ${match[2]}` : match[1],
      };
  }
  return { primary: item, secondary: "" };
}

/** Which edges of a scroll area hide more content (1px slack for zoom rounding). */
export function scrollFades(area: {
  scrollTop: number;
  scrollHeight: number;
  clientHeight: number;
}): { top: boolean; bottom: boolean } {
  const rest = area.scrollHeight - area.clientHeight - area.scrollTop;
  return { top: area.scrollTop > 1, bottom: rest > 1 };
}

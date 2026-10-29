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
    | "termination.k_impact_ports_other"
    | "termination.k_impact_critical"
    | "termination.k_impact_service";
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
  if (impact.critical)
    parts.push({
      key: "termination.k_impact_critical",
      values: {},
      warn: true,
    });
  if (impact.service)
    parts.push({
      key: "termination.k_impact_service",
      values: { service: impact.service },
      warn: true,
    });
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

export { scrollFades } from "./ScrollArea";

/** Rows each Details group shows before "Show all". */
export const IMPACT_PREVIEW = 5;
/** Expanded groups longer than this get a filter field. */
export const IMPACT_FILTER_FROM = 20;

/**
 * What one Details group shows. Collapsed groups preview a few rows and
 * count the rest; expanded ones list every kept row, narrowed by `query`
 * (case-insensitive, any order of words). `unlisted` counts rows Rust left
 * out of the payload, which can never be shown.
 */
export function impactGroupView(
  list: CappedList,
  expanded: boolean,
  query = "",
): {
  items: { item: string; index: number }[];
  hidden: number;
  expandable: boolean;
  filterable: boolean;
  unlisted: number;
} {
  const total = listTotal(list);
  const expandable = total > IMPACT_PREVIEW;
  const indexed = list.items.map((item, index) => ({ item, index }));
  if (!expanded || !expandable) {
    const items = indexed.slice(0, IMPACT_PREVIEW);
    return {
      items,
      hidden: total - items.length,
      expandable,
      filterable: false,
      unlisted: 0,
    };
  }
  const words = query.toLocaleLowerCase().split(/\s+/).filter(Boolean);
  const items = words.length
    ? indexed.filter(({ item }) => {
        const text = item.toLocaleLowerCase();
        return words.every((word) => text.includes(word));
      })
    : indexed;
  return {
    items,
    hidden: 0,
    expandable,
    filterable: total > IMPACT_FILTER_FROM,
    unlisted: list.more,
  };
}

/**
 * Message key for the confirmation title, note or button. One target is
 * named ("Close Blender?"); several are counted ("Close 3 processes?").
 * Force uses "Force quit" throughout so the button matches the title.
 */
export function confirmationText(
  confirmation: Pick<Confirmation, "force" | "targets">,
  part: "title" | "note" | "button",
) {
  const mode = confirmation.force ? "force" : "confirm";
  const count = confirmation.targets.length === 1 ? "one" : "many";
  return `termination.k_${mode}_${part}_${count}` as const;
}

/** Values for {@link confirmationText}; long names are shortened for buttons. */
export function confirmationValues(
  confirmation: Pick<Confirmation, "targets">,
  maxName = Infinity,
) {
  const first = confirmation.targets[0];
  let name = first?.name || (first ? `PID ${first.pid}` : "");
  if (name.length > maxName) name = `${name.slice(0, maxName - 1).trimEnd()}…`;
  return { name, count: confirmation.targets.length };
}

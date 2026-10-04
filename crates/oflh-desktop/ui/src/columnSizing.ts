import type { Row } from "./api";
import type { ColumnKey } from "./Table";
import { compactPath, memory } from "./state";
import { t, tValue } from "./i18n";

/** Text actually shown in each column, including translated labels and badges. */
export function columnText(
  row: Row,
  column: ColumnKey,
  handles: boolean,
  target: string,
): string {
  switch (column) {
    case "process":
      return row.name || t("app.k_unnamed");
    case "pid":
      return String(row.pid || "—");
    case "path":
      return (
        ((handles ? row.path : compactPath(row.path, target)) ||
          t("common.k_unavailable_2c9c1f79")) +
        (row.deleted ? ` ${t("table.k_deleted")}` : "")
      );
    case "evidence":
      return row.evidence
        ? tValue(row.evidence_label ?? "")
        : handles
          ? `${tValue(row.relation)} · ${tValue(row.access)}`
          : `${row.usages} ${t("status.k_file_usages_d01933d6")}`;
    case "memory":
      return memory(row.memory);
    case "cpu":
      return row.cpu === null ? "—" : `${row.cpu.toFixed(1)}%`;
    case "address":
      return row.port?.address ?? "—";
    case "port":
      return String(row.port?.number ?? "—");
    case "protocol":
      return `${row.port?.protocol ?? ""} ${tValue(row.port?.state ?? "")}`;
  }
}

/** Measure with the current rendered fonts instead of assuming character widths. */
export function columnMeasurer(
  grid: HTMLElement,
  index: number,
  column: ColumnKey,
  heading: string,
) {
  const canvas = document.createElement("canvas");
  const context = canvas.getContext("2d");
  if (!context) throw new Error("Text measurement is unavailable");
  const header = grid.querySelector<HTMLElement>(
    `[role="columnheader"]:nth-child(${index + 1})`,
  )!;
  const cell = grid.querySelector<HTMLElement>(
    `.data-row [role="gridcell"]:nth-child(${index + 1})`,
  );
  const headerStyle = getComputedStyle(header);
  const bodyStyle = getComputedStyle(cell ?? grid);
  const measure = (text: string, style: CSSStyleDeclaration) => {
    context.font = `${style.fontWeight} ${style.fontSize} ${style.fontFamily}`;
    return (
      context.measureText(text).width +
      Math.max(0, text.length - 1) * (parseFloat(style.letterSpacing) || 0)
    );
  };
  // Reserve header padding and a sort indicator, even when currently unsorted.
  let width = Math.max(70, measure(heading, headerStyle) + 48);
  return {
    include(text: string) {
      const decoration =
        column === "process"
          ? 42
          : column === "protocol"
            ? 18
            : column === "evidence"
              ? 20
              : column === "path"
                ? 16
                : 0;
      width = Math.max(width, measure(text, bodyStyle) + 24 + decoration);
    },
    width: () => Math.ceil(width),
  };
}

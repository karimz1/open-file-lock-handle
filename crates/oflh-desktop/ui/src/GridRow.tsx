import { memo } from "react";
import { PanelRightClose, PanelRightOpen, ShieldCheck } from "lucide-react";
import { RowIcon } from "./AppIcon";
import type { Row } from "./api";
import { useLocale, t, tValue } from "./i18n";
import { memory, compactPath } from "./state";
import { sessionCell } from "./sessions";
import type { ColumnKey } from "./Table";

interface GridRowProps {
  index: number;
  start: number;
  row: Row | undefined;
  selected: boolean;
  focused: boolean;
  cursor: boolean;
  expanded: boolean;
  template: string;
  rowHeight: number;
  hiddenColumns: Set<ColumnKey>;
  ports: boolean;
  handles: boolean;
  target: string;
  revision: number | undefined;
  measure: (element: HTMLElement | null) => void;
  onPick: (index: number, row: Row, additive: boolean) => void;
  onOpen: (row: Row) => void;
  onContext: (row: Row) => void;
  onToggle: (row: Row) => void;
}

/**
 * One grid row. It is memoized, so scrolling, moving the selection or typing
 * elsewhere only renders the rows whose own props changed. Every callback is
 * stable and every other prop is a primitive or a long-lived object.
 */
export const GridRow = memo(function GridRow({
  index,
  start,
  row,
  selected,
  focused,
  cursor,
  expanded,
  template,
  rowHeight,
  hiddenColumns,
  ports,
  handles,
  target,
  revision,
  measure,
  onPick,
  onOpen,
  onContext,
  onToggle,
}: GridRowProps) {
  // The text is translated while rendering, so a language switch must reach a
  // row that is otherwise memoized.
  useLocale();
  return (
    <div
      id={`result-row-${index}`}
      role="row"
      data-cursor={cursor}
      aria-rowindex={index + 2}
      aria-selected={selected}
      className={`data-row ${selected ? "selected" : ""} ${focused ? "focused" : ""}`}
      data-index={index}
      ref={measure}
      style={{
        position: "absolute",
        top: start,
        minHeight: rowHeight,
        gridTemplateColumns: template,
        width: "100%",
      }}
      onClick={(event) => {
        if (row) {
          onPick(index, row, event.ctrlKey || event.metaKey || event.shiftKey);
        }
      }}
      onDoubleClick={() => row && onOpen(row)}
      onContextMenu={(event) => {
        if (row) {
          event.preventDefault();
          onContext(row);
        }
      }}
    >
      {row ? (
        <>
          <div role="gridcell" className="process-cell">
            <button
              type="button"
              className="process-icon row-details-toggle"
              aria-label={`${t(expanded ? "common.k_close_details" : "common.k_open_details")} ${t("app.k_details_for")} ${row.name || t("app.k_process_c2e2d662")}${row.port ? ` ${row.port.endpoint}` : ` ${compactPath(row.path, target)}`}`}
              title={
                expanded
                  ? t("app.k_close_details_panel")
                  : t("app.k_open_details_panel")
              }
              aria-expanded={expanded}
              onClick={(event) => {
                event.stopPropagation();
                onToggle(row);
              }}
              onDoubleClick={(event) => event.stopPropagation()}
            >
              {expanded ? (
                <PanelRightClose size={14} />
              ) : (
                <PanelRightOpen size={14} />
              )}
            </button>
            <RowIcon
              revision={revision}
              reference={row.icon_ref}
              processKey={row.process_key}
            />
            <span title={row.name}>{row.name || t("app.k_unnamed")}</span>
            {selected && <span className="selection-mark" />}
          </div>
          {!hiddenColumns.has("pid") && (
            <div role="gridcell" className="mono muted">
              {row.pid || "—"}
            </div>
          )}
          {!hiddenColumns.has("user") && (
            <div role="gridcell" title={row.user}>
              {row.user || "—"}
            </div>
          )}
          {!hiddenColumns.has("session") && (
            <div role="gridcell" className="muted">
              {sessionCell(row.session, row.session_kind)}
            </div>
          )}
          {ports ? (
            <>
              {!hiddenColumns.has("address") && (
                <div
                  role="gridcell"
                  className="mono"
                  title={row.port?.endpoint}
                >
                  {row.port?.address ?? "—"}
                </div>
              )}
              {!hiddenColumns.has("port") && (
                <div role="gridcell" className="mono">
                  {row.port?.number ?? "—"}
                </div>
              )}
              {!hiddenColumns.has("protocol") && (
                <div role="gridcell">
                  <span className="badge">{row.port?.protocol}</span>{" "}
                  <span className="muted">{tValue(row.port?.state ?? "")}</span>
                </div>
              )}
            </>
          ) : (
            <>
              {!hiddenColumns.has("path") && (
                <div
                  role="gridcell"
                  className={`mono path-cell ${handles || row.matching_files ? "full-path-cell" : ""}`}
                  title={row.path}
                >
                  {(handles || row.matching_files
                    ? row.path
                    : compactPath(row.path, target)) ||
                    t("common.k_unavailable_2c9c1f79")}
                  {row.deleted && (
                    <span className="badge">{t("table.k_deleted")}</span>
                  )}
                  {row.matching_files && row.matching_files.total > 1 && (
                    <button
                      type="button"
                      className="inline-link matching-files-more"
                      aria-label={t("matches.k_show_all_count", {
                        n: row.matching_files.total,
                      })}
                      onClick={(event) => {
                        event.stopPropagation();
                        onOpen(row);
                      }}
                      onDoubleClick={(event) => event.stopPropagation()}
                    >
                      {t("matches.k_more", {
                        n: row.matching_files.total - 1,
                      })}
                    </button>
                  )}
                </div>
              )}
              {!hiddenColumns.has("evidence") && (
                <div role="gridcell" title={row.evidence ?? undefined}>
                  {row.evidence ? (
                    <span className="evidence">
                      <ShieldCheck size={13} />
                      {tValue(row.evidence_label ?? "")}
                    </span>
                  ) : (
                    <span className="muted">
                      {handles
                        ? `${tValue(row.relation)} · ${tValue(row.access)}`
                        : `${row.usages} ${t("status.k_file_usages_d01933d6")}`}
                    </span>
                  )}
                </div>
              )}
            </>
          )}
          {!hiddenColumns.has("memory") && (
            <div role="gridcell" className="mono numeric">
              {memory(row.memory)}
            </div>
          )}
          {!hiddenColumns.has("cpu") && (
            <div role="gridcell" className="mono numeric">
              {row.cpu === null ? "—" : `${row.cpu.toFixed(1)}%`}
            </div>
          )}
        </>
      ) : (
        <div role="gridcell" className="muted">
          {t("app.k_loading")}…
        </div>
      )}
    </div>
  );
});

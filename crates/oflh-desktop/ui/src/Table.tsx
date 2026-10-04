import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import {
  ArrowDown,
  ArrowUp,
  PanelRightOpen,
  PanelRightClose,
  FileSearch,
  Search,
  ShieldCheck,
} from "lucide-react";
import { api, type Page, type Row, type Sort, type TableQuery } from "./api";
import { columnText, columnMeasurer } from "./columnSizing";
import { memory, compactPath } from "./state";
import { t, tValue, type MessageKey } from "./i18n";
export type ColumnKey =
  | "process"
  | "pid"
  | "path"
  | "evidence"
  | "memory"
  | "cpu"
  | "address"
  | "port"
  | "protocol";
export interface ColumnDefinition {
  key: ColumnKey;
  label: MessageKey;
  sort?: Sort;
  width: number;
}
interface Props {
  fontSize: number;
  target: string;
  expandedKey: string | null;
  onToggle: (row: Row) => void;
  revision: number;
  query: TableQuery;
  hiddenColumns: Set<ColumnKey>;
  selected: Set<string>;
  focused: string | null;
  onSelect: (row: Row, additive: boolean) => void;
  onOpen: (row: Row) => void;
  onContext: (row: Row) => void;
  onSort: (sort: Sort) => void;
  onTotal: (total: number) => void;
  onError: (error: unknown) => void;
}
export const fileColumns: ColumnDefinition[] = [
  { key: "process", label: "app.k_process", sort: "name", width: 220 },
  { key: "pid", label: "app.k_pid", sort: "pid", width: 84 },
  { key: "path", label: "app.k_path", sort: "path", width: 380 },
  { key: "evidence", label: "filters.k_evidence_access", width: 170 },
  { key: "memory", label: "inspector.k_memory", sort: "memory", width: 104 },
  { key: "cpu", label: "app.k_cpu", sort: "cpu", width: 80 },
];
export const portColumns: ColumnDefinition[] = [
  { key: "process", label: "app.k_process", sort: "name", width: 220 },
  { key: "pid", label: "app.k_pid", sort: "pid", width: 84 },
  { key: "address", label: "app.k_local_address", sort: "address", width: 230 },
  { key: "port", label: "app.k_port", sort: "port", width: 90 },
  {
    key: "protocol",
    label: "table.k_protocol_state",
    sort: "protocol",
    width: 170,
  },
  { key: "memory", label: "inspector.k_memory", sort: "memory", width: 104 },
  { key: "cpu", label: "app.k_cpu", sort: "cpu", width: 80 },
];
export function Table(props: Props) {
  const allColumns = props.query.ports ? portColumns : fileColumns;
  const columns = allColumns.filter(
    (column) =>
      column.key === "process" || !props.hiddenColumns.has(column.key),
  );
  const fitRequest = useRef(0);
  const [fitting, setFitting] = useState<ColumnKey | null>(null);
  const scroll = useRef<HTMLDivElement>(null);
  const resizing = useRef<{
    key: ColumnKey;
    pointerId: number;
    startX: number;
    startWidth: number;
  } | null>(null);
  const [page, setPage] = useState<
    (Page & { offset: number; queryKey: string }) | null
  >(null);
  const [cursor, setCursor] = useState(0);
  const pendingNavigation = useRef<{ index: number; additive: boolean } | null>(
    null,
  );
  const [widths, setWidths] = useState<Record<ColumnKey, number>>({
    process: 220,
    pid: 84,
    path: 380,
    evidence: 170,
    memory: 104,
    cpu: 80,
    address: 230,
    port: 90,
    protocol: 170,
  });
  const widthFor = (column: ColumnDefinition) =>
    widths[column.key] ?? column.width;
  const template = columns.map((column) => `${widthFor(column)}px`).join(" ");
  const minWidth = columns.reduce((sum, column) => sum + widthFor(column), 0);
  const rowHeight = Math.round((38 * props.fontSize) / 13);
  const virtual = useVirtualizer({
    count: page?.total ?? 0,
    getScrollElement: () => scroll.current,
    estimateSize: () => rowHeight,
    overscan: 10,
  });
  useEffect(() => {
    virtual.measure();
  }, [props.fontSize, props.query.handles]);
  const items = virtual.getVirtualItems();
  const offset = Math.floor((items[0]?.index ?? 0) / 100) * 100;
  const queryKey = JSON.stringify({ ...props.query, offset: 0 });
  useEffect(() => {
    scroll.current?.scrollTo({ top: 0 });
    setCursor(0);
    pendingNavigation.current = null;
  }, [queryKey]);
  useEffect(() => {
    let active = true;
    const timer = setTimeout(() => {
      api
        .page(props.revision, { ...props.query, offset, limit: 200 })
        .then((result) => {
          if (active) {
            setPage({ ...result, offset, queryKey });
            props.onTotal(result.total);
          }
        })
        .catch((error) => {
          if (active) props.onError(error);
        });
    }, 45);
    return () => {
      active = false;
      clearTimeout(timer);
    };
  }, [props.revision, queryKey, offset]); // Snapshot/compiled query/viewport are the request identity.
  useEffect(() => {
    const pending = pendingNavigation.current;
    const row = pending && page?.rows[pending.index - page.offset];
    if (pending && row) {
      pendingNavigation.current = null;
      props.onSelect(row, pending.additive);
    }
  }, [page]);
  useEffect(() => {
    // Discard measurements after filters, snapshot, view, or font changes.
    fitRequest.current++;
    setFitting(null);
  }, [
    props.revision,
    queryKey,
    props.fontSize,
    props.target,
    props.hiddenColumns,
  ]);
  useEffect(
    () => () => {
      fitRequest.current++;
    },
    [],
  );
  const autoFit = async (column: ColumnDefinition) => {
    if (!scroll.current) return;
    const request = ++fitRequest.current;
    setFitting(column.key);
    try {
      await document.fonts.ready;
      if (request !== fitRequest.current || !scroll.current) return;
      const measurement = columnMeasurer(
        scroll.current,
        columns.indexOf(column),
        column.key,
        t(column.label),
      );
      let offset = 0;
      // The grid is virtualized. Walk bounded IPC pages so offscreen matches
      // participate without creating DOM nodes for the complete snapshot.
      do {
        const result =
          page?.offset === offset &&
          page.revision === props.revision &&
          page.queryKey === queryKey
            ? page
            : await api.page(props.revision, {
                ...props.query,
                offset,
                limit: 200,
              });
        if (request !== fitRequest.current) return;
        if (result.revision !== props.revision) return;
        for (const row of result.rows)
          measurement.include(
            columnText(row, column.key, props.query.handles, props.target),
          );
        offset += result.rows.length;
        if (!result.rows.length || offset >= result.total) break;
        // Keep navigation and cancellation responsive between pages.
        await new Promise<void>((resolve) => setTimeout(resolve, 0));
      } while (request === fitRequest.current);
      if (request === fitRequest.current)
        setWidths((current) => ({
          ...current,
          [column.key]: measurement.width(),
        }));
    } catch (error) {
      if (request === fitRequest.current) props.onError(error);
    } finally {
      if (request === fitRequest.current) setFitting(null);
    }
  };
  const rowAt = (index: number) => page?.rows[index - page.offset];
  const navigate = (event: KeyboardEvent<HTMLDivElement>) => {
    let index = cursor;
    if (event.key === "ArrowDown") index++;
    else if (event.key === "ArrowUp") index--;
    else if (event.key === "Home") index = 0;
    else if (event.key === "End") index = (page?.total ?? 1) - 1;
    else if (event.key === "Enter") {
      const row = rowAt(cursor);
      if (row) props.onOpen(row);
      event.preventDefault();
      return;
    } else if (event.key === " ") {
      const row = rowAt(cursor);
      if (row) props.onSelect(row, true);
      event.preventDefault();
      return;
    } else if (event.key === "F10" && event.shiftKey) {
      const row = rowAt(cursor);
      if (row) props.onContext(row);
      event.preventDefault();
      return;
    } else return;
    event.preventDefault();
    index = Math.max(0, Math.min((page?.total ?? 1) - 1, index));
    setCursor(index);
    virtual.scrollToIndex(index);
    const row = rowAt(index);
    if (row) props.onSelect(row, event.shiftKey);
    else pendingNavigation.current = { index, additive: event.shiftKey };
  };
  return (
    <div
      className="table-scroll"
      ref={scroll}
      role="grid"
      aria-label={
        props.query.ports
          ? t("inspector.k_local_tcp_listeners_and_udp_bindings")
          : props.query.handles
            ? t("inspector.k_matching_file_usages_selection_applies_56df1d76")
            : t("inspector.k_processes_using_this_target")
      }
      aria-rowcount={(page?.total ?? 0) + 1}
      aria-colcount={columns.length}
      aria-multiselectable
      aria-activedescendant={
        items.some((item) => item.index === cursor)
          ? `result-row-${cursor}`
          : undefined
      }
      tabIndex={0}
      onKeyDown={navigate}
    >
      <div
        className="table-header"
        role="row"
        style={{
          gridTemplateColumns: template,
          minWidth,
        }}
      >
        {columns.map((column) => (
          <div
            role="columnheader"
            className="table-header-cell"
            key={column.key}
            aria-sort={
              column.sort && props.query.sort === column.sort
                ? props.query.descending
                  ? "descending"
                  : "ascending"
                : undefined
            }
          >
            {column.sort ? (
              <button
                className="sort-header"
                type="button"
                aria-label={`${t("table.k_sort_by")} ${t(column.label)}`}
                onClick={() => props.onSort(column.sort!)}
              >
                <span>{t(column.label)}</span>
                {props.query.sort === column.sort && (
                  <span className="sort-indicator" aria-hidden="true">
                    {props.query.descending ? (
                      <ArrowDown size={13} />
                    ) : (
                      <ArrowUp size={13} />
                    )}
                  </span>
                )}
              </button>
            ) : (
              <span className="table-header-label">{t(column.label)}</span>
            )}
            <span
              role="separator"
              aria-label={t("table.k_resize_column_column", {
                column: t(column.label),
              })}
              aria-orientation="vertical"
              aria-valuenow={widthFor(column)}
              aria-valuemin={70}
              aria-busy={fitting === column.key}
              title={t("table.k_auto_fit_hint")}
              onDoubleClick={(event) => {
                event.preventDefault();
                event.stopPropagation();
                void autoFit(column);
              }}
              tabIndex={0}
              className="resize-handle"
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  event.preventDefault();
                  event.stopPropagation();
                  void autoFit(column);
                  return;
                }
                if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
                  event.preventDefault();
                  event.stopPropagation();
                  fitRequest.current++;
                  setFitting(null);
                  setWidths((current) => ({
                    ...current,
                    [column.key]: Math.max(
                      70,
                      widthFor(column) +
                        (event.key === "ArrowRight" ? 16 : -16),
                    ),
                  }));
                }
              }}
              onPointerDown={(event) => {
                event.preventDefault();
                event.stopPropagation();
                fitRequest.current++;
                setFitting(null);
                event.currentTarget.setPointerCapture(event.pointerId);
                resizing.current = {
                  key: column.key,
                  pointerId: event.pointerId,
                  startX: event.clientX,
                  startWidth: widthFor(column),
                };
              }}
              onPointerMove={(event) => {
                const active = resizing.current;
                if (!active || active.pointerId !== event.pointerId) return;
                setWidths((current) => ({
                  ...current,
                  [active.key]: Math.max(
                    70,
                    active.startWidth + event.clientX - active.startX,
                  ),
                }));
              }}
              onPointerUp={() => {
                resizing.current = null;
              }}
              onPointerCancel={() => {
                resizing.current = null;
              }}
            />
          </div>
        ))}
      </div>
      {!page ? (
        <div className="empty">
          <Search size={28} />
          <h3>{t("table.k_loading_results")}</h3>
        </div>
      ) : page.total === 0 ? (
        <div className="empty">
          <FileSearch size={32} />
          <h3>
            {props.query.ports
              ? t("table.k_no_matching_local_ports")
              : t("table.k_no_matching_processes")}
          </h3>
          <p>
            {props.query.text ||
            props.query.locks_only ||
            Object.values(props.query.columns || {}).some(
              (value) => value !== undefined && value !== "" && value !== "any",
            ) ||
            props.query.process_key
              ? t("filters.k_no_rows_match_the_current_filters_clear_0d9d0c2e")
              : t("table.k_no_visible_process_references_this_targ_b434e8b6")}
          </p>
        </div>
      ) : (
        <div
          style={{
            height: virtual.getTotalSize(),
            minWidth,
            position: "relative",
          }}
        >
          {items.map((item) => {
            const row = rowAt(item.index);
            return (
              <div
                key={item.key}
                id={`result-row-${item.index}`}
                role="row"
                data-cursor={item.index === cursor}
                aria-rowindex={item.index + 2}
                aria-selected={!!row && props.selected.has(row.process_key)}
                className={`data-row ${row && props.selected.has(row.process_key) ? "selected" : ""} ${row?.process_key === props.focused ? "focused" : ""}`}
                data-index={item.index}
                ref={virtual.measureElement}
                style={{
                  position: "absolute",
                  top: item.start,
                  minHeight: rowHeight,
                  gridTemplateColumns: template,
                  width: "100%",
                }}
                onClick={(event) => {
                  if (row) {
                    setCursor(item.index);
                    props.onSelect(
                      row,
                      event.ctrlKey || event.metaKey || event.shiftKey,
                    );
                  }
                }}
                onDoubleClick={() => row && props.onOpen(row)}
                onContextMenu={(event) => {
                  if (row) {
                    event.preventDefault();
                    props.onContext(row);
                  }
                }}
              >
                {row ? (
                  <>
                    <div role="gridcell" className="process-cell">
                      <button
                        type="button"
                        className="process-icon row-details-toggle"
                        aria-label={`${t(props.expandedKey === row.key ? "common.k_close_details" : "common.k_open_details")} ${t("app.k_details_for")} ${row.name || t("app.k_process_c2e2d662")}${row.port ? ` ${row.port.endpoint}` : ` ${compactPath(row.path, props.target)}`}`}
                        title={
                          props.expandedKey === row.key
                            ? t("app.k_close_details_panel")
                            : t("app.k_open_details_panel")
                        }
                        aria-expanded={props.expandedKey === row.key}
                        onClick={(event) => {
                          event.stopPropagation();
                          props.onToggle(row);
                        }}
                        onDoubleClick={(event) => event.stopPropagation()}
                      >
                        {props.expandedKey === row.key ? (
                          <PanelRightClose size={14} />
                        ) : (
                          <PanelRightOpen size={14} />
                        )}
                      </button>
                      <span title={row.name}>
                        {row.name || t("app.k_unnamed")}
                      </span>
                      {props.selected.has(row.process_key) && (
                        <span className="selection-mark" />
                      )}
                    </div>
                    {!props.hiddenColumns.has("pid") && (
                      <div role="gridcell" className="mono muted">
                        {row.pid || "—"}
                      </div>
                    )}
                    {props.query.ports ? (
                      <>
                        {!props.hiddenColumns.has("address") && (
                          <div
                            role="gridcell"
                            className="mono"
                            title={row.port?.endpoint}
                          >
                            {row.port?.address ?? "—"}
                          </div>
                        )}
                        {!props.hiddenColumns.has("port") && (
                          <div role="gridcell" className="mono">
                            {row.port?.number ?? "—"}
                          </div>
                        )}
                        {!props.hiddenColumns.has("protocol") && (
                          <div role="gridcell">
                            <span className="badge">{row.port?.protocol}</span>{" "}
                            <span className="muted">
                              {tValue(row.port?.state ?? "")}
                            </span>
                          </div>
                        )}
                      </>
                    ) : (
                      <>
                        {!props.hiddenColumns.has("path") && (
                          <div
                            role="gridcell"
                            className={`mono path-cell ${props.query.handles ? "full-path-cell" : ""}`}
                            title={row.path}
                          >
                            {(props.query.handles
                              ? row.path
                              : compactPath(row.path, props.target)) ||
                              t("common.k_unavailable_2c9c1f79")}
                            {row.deleted && (
                              <span className="badge">
                                {t("table.k_deleted")}
                              </span>
                            )}
                          </div>
                        )}
                        {!props.hiddenColumns.has("evidence") && (
                          <div
                            role="gridcell"
                            title={row.evidence ?? undefined}
                          >
                            {row.evidence ? (
                              <span className="evidence">
                                <ShieldCheck size={13} />
                                {tValue(row.evidence_label ?? "")}
                              </span>
                            ) : (
                              <span className="muted">
                                {props.query.handles
                                  ? `${tValue(row.relation)} · ${tValue(row.access)}`
                                  : `${row.usages} ${t("status.k_file_usages_d01933d6")}`}
                              </span>
                            )}
                          </div>
                        )}
                      </>
                    )}
                    {!props.hiddenColumns.has("memory") && (
                      <div role="gridcell" className="mono numeric">
                        {memory(row.memory)}
                      </div>
                    )}
                    {!props.hiddenColumns.has("cpu") && (
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
          })}
        </div>
      )}
    </div>
  );
}

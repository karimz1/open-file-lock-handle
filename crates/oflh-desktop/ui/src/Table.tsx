import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
} from "react";
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
import { memory, compactPath } from "./state";
interface Props {
  fontSize: number;
  target: string;
  expandedKey: string | null;
  onToggle: (row: Row) => void;
  revision: number;
  query: TableQuery;
  selected: Set<string>;
  focused: string | null;
  onSelect: (row: Row, additive: boolean) => void;
  onOpen: (row: Row) => void;
  onContext: (row: Row) => void;
  onSort: (sort: Sort) => void;
  onTotal: (total: number) => void;
  onError: (error: unknown) => void;
}
const fileColumns: { label: string; sort?: Sort; width: number }[] = [
  { label: "Process", sort: "name", width: 220 },
  { label: "PID", sort: "pid", width: 84 },
  { label: "Path", sort: "path", width: 350 },
  { label: "Evidence / access", width: 170 },
  { label: "Memory", sort: "memory", width: 104 },
  { label: "CPU", sort: "cpu", width: 80 },
];
const portColumns: { label: string; sort?: Sort; width: number }[] = [
  { label: "Process", sort: "name", width: 220 },
  { label: "PID", sort: "pid", width: 84 },
  { label: "Local address", sort: "address", width: 230 },
  { label: "Port", sort: "port", width: 90 },
  { label: "Protocol / state", sort: "protocol", width: 170 },
  { label: "Memory", sort: "memory", width: 104 },
  { label: "CPU", sort: "cpu", width: 80 },
];
export function Table(props: Props) {
  const columns = props.query.ports ? portColumns : fileColumns;
  const scroll = useRef<HTMLDivElement>(null);
  const [page, setPage] = useState<(Page & { offset: number }) | null>(null);
  const [cursor, setCursor] = useState(0);
  const pendingNavigation = useRef<{ index: number; additive: boolean } | null>(
    null,
  );
  const [widths, setWidths] = useState(columns.map((column) => column.width));
  const virtual = useVirtualizer({
    count: page?.total ?? 0,
    getScrollElement: () => scroll.current,
    estimateSize: () => Math.round((38 * props.fontSize) / 13),
    overscan: 10,
  });
  useEffect(() => {
    virtual.measure();
  }, [props.fontSize]);
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
            setPage({ ...result, offset });
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
  const template = useMemo(
    () => widths.map((width) => `${width}px`).join(" "),
    [widths],
  );
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
          ? "Local TCP listeners and UDP bindings"
          : props.query.handles
            ? "Matching file usages; selection applies to processes"
            : "Processes using this target"
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
          minWidth: widths.reduce((sum, width) => sum + width, 0),
        }}
      >
        {columns.map((column, index) => (
          <div
            role="columnheader"
            key={column.label}
            aria-sort={
              column.sort && props.query.sort === column.sort
                ? props.query.descending
                  ? "descending"
                  : "ascending"
                : undefined
            }
          >
            {column.sort ? (
              <button onClick={() => props.onSort(column.sort!)}>
                {column.label}
                {props.query.sort === column.sort &&
                  (props.query.descending ? (
                    <ArrowDown size={12} />
                  ) : (
                    <ArrowUp size={12} />
                  ))}
              </button>
            ) : (
              <span>{column.label}</span>
            )}
            <span
              role="separator"
              aria-label={`Resize ${column.label} column`}
              aria-orientation="vertical"
              aria-valuenow={widths[index]}
              tabIndex={0}
              className="resize-handle"
              onKeyDown={(event) => {
                if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
                  event.preventDefault();
                  event.stopPropagation();
                  setWidths((current) =>
                    current.map((width, position) =>
                      position === index
                        ? Math.max(
                            70,
                            width + (event.key === "ArrowRight" ? 16 : -16),
                          )
                        : width,
                    ),
                  );
                }
              }}
              onPointerDown={(event) => {
                event.preventDefault();
                const handle = event.currentTarget;
                handle.setPointerCapture(event.pointerId);
                const start = event.clientX;
                const initial = widths[index];
                handle.onpointermove = (move) =>
                  setWidths((current) =>
                    current.map((width, position) =>
                      position === index
                        ? Math.max(70, initial + move.clientX - start)
                        : width,
                    ),
                  );
                handle.onpointerup = () => {
                  handle.onpointermove = null;
                  handle.onpointerup = null;
                };
              }}
            />
          </div>
        ))}
      </div>
      {!page ? (
        <div className="empty">
          <Search size={28} />
          <h3>Loading results</h3>
        </div>
      ) : page.total === 0 ? (
        <div className="empty">
          <FileSearch size={32} />
          <h3>
            {props.query.ports
              ? "No matching local ports"
              : "No matching processes"}
          </h3>
          <p>
            {props.query.text ||
            props.query.locks_only ||
            Object.values(props.query.columns || {}).some(
              (value) => value !== undefined && value !== "" && value !== "any",
            ) ||
            props.query.process_key
              ? "No rows match the current filters. Clear a filter to broaden the view."
              : "No visible process references this target. Permission limits may hide some usage."}
          </p>
        </div>
      ) : (
        <div
          style={{
            height: virtual.getTotalSize(),
            minWidth: widths.reduce((sum, width) => sum + width, 0),
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
                style={{
                  position: "absolute",
                  top: item.start,
                  height: item.size,
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
                        aria-label={`${props.expandedKey === row.key ? "Close" : "Open"} details for ${row.name || "process"}${row.port ? ` ${row.port.endpoint}` : ` ${compactPath(row.path, props.target)}`}`}
                        title={
                          props.expandedKey === row.key
                            ? "Close details panel"
                            : "Open details panel"
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
                      <span title={row.name}>{row.name || "(unnamed)"}</span>
                      {props.selected.has(row.process_key) && (
                        <span className="selection-mark" />
                      )}
                    </div>
                    <div role="gridcell" className="mono muted">
                      {row.pid || "—"}
                    </div>
                    {props.query.ports ? (
                      <>
                        <div
                          role="gridcell"
                          className="mono"
                          title={row.port?.endpoint}
                        >
                          {row.port?.address ?? "—"}
                        </div>
                        <div role="gridcell" className="mono">
                          {row.port?.number ?? "—"}
                        </div>
                        <div role="gridcell">
                          <span className="badge">{row.port?.protocol}</span>{" "}
                          <span className="muted">{row.port?.state}</span>
                        </div>
                      </>
                    ) : (
                      <>
                        <div
                          role="gridcell"
                          className="mono path-cell"
                          title={row.path}
                        >
                          {compactPath(row.path, props.target) || "Unavailable"}
                          {row.deleted && (
                            <span className="badge">deleted</span>
                          )}
                        </div>
                        <div role="gridcell" title={row.evidence ?? undefined}>
                          {row.evidence ? (
                            <span className="evidence">
                              <ShieldCheck size={13} />
                              {row.evidence_label}
                            </span>
                          ) : (
                            <span className="muted">
                              {props.query.handles
                                ? `${row.relation} · ${row.access}`
                                : `${row.usages} file usages`}
                            </span>
                          )}
                        </div>
                      </>
                    )}
                    <div role="gridcell" className="mono numeric">
                      {memory(row.memory)}
                    </div>
                    <div role="gridcell" className="mono numeric">
                      {row.cpu === null ? "—" : `${row.cpu.toFixed(1)}%`}
                    </div>
                  </>
                ) : (
                  <div role="gridcell" className="muted">
                    Loading…
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

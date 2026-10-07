import { useEffect, useRef, useState } from "react";
import {
  Copy,
  ExternalLink,
  Files,
  ChevronRight,
  CornerDownRight,
  X,
} from "lucide-react";
import { type Details, type PathValue, type Row } from "./api";
import { memory } from "./state";
import { t, tValue, type MessageKey } from "./i18n";
export function Inspector({
  details,
  availability,
  rowCurrent,
  row,
  close,
  copy,
  reveal,
  handles,
  ports,
  inspectFolder,
  terminateAncestor,
  terminateCurrent,
}: {
  details: Details;
  availability: "current" | "updating" | "missing";
  rowCurrent: boolean;
  row: Row | null;
  close: () => void;
  copy: (field: string, reference?: string) => void;
  reveal: (reference: string) => void;
  handles: () => void;
  ports: () => void;
  inspectFolder: () => void;
  terminateAncestor: (key: string, force: boolean) => void;
  terminateCurrent: (force: boolean) => void;
}) {
  const current = availability === "current";
  const panel = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(() => {
    try {
      const value = Number(localStorage.getItem("oflh-details-width"));
      return value >= 260 && value <= 720 ? value : 340;
    } catch {
      return 340;
    }
  });
  const [maximum, setMaximum] = useState(720);
  const actualWidth = Math.min(width, maximum);
  const drag = useRef<{ x: number; width: number } | null>(null);
  useEffect(() => {
    const parent = panel.current?.parentElement;
    if (!parent) return;
    const observer = new ResizeObserver(() =>
      setMaximum(Math.max(260, Math.min(720, parent.clientWidth - 280))),
    );
    observer.observe(parent);
    return () => observer.disconnect();
  }, []);
  useEffect(() => {
    try {
      localStorage.setItem("oflh-details-width", String(width));
    } catch {
      /* Keep the session preference. */
    }
  }, [width]);
  const resize = (value: number) =>
    setWidth(Math.max(260, Math.min(maximum, value)));
  const [ancestorKey, setAncestorKey] = useState<string>(
    details.process.process_key,
  );
  const nodes = [...details.ancestors].reverse().concat({
    key: details.process.process_key,
    name: details.process.name,
    pid: details.process.pid,
    actionable: details.process.actionable,
  });
  const ancestor = nodes.find((item) => item.key === ancestorKey);
  const currentSelected = ancestorKey === details.process.process_key;
  const path = (label: MessageKey, value: PathValue, available = current) => (
    <section>
      <h4>{t(label)}</h4>
      <p className="detail-path mono">
        {value.display || t("common.k_unavailable_2c9c1f79")}
      </p>
      {value.display && (
        <div className="inline-actions">
          <button
            disabled={!available}
            onClick={() => copy("path", value.reference)}
          >
            <Copy size={13} />
            {t("selection.k_copy_path")}
          </button>
          <button disabled={!available} onClick={() => reveal(value.reference)}>
            <ExternalLink size={13} />
            {t("common.k_reveal")}
          </button>
        </div>
      )}
    </section>
  );
  return (
    <div
      className="inspector-container"
      ref={panel}
      style={{ width: actualWidth }}
    >
      <div
        role="separator"
        aria-label={t("inspector.k_resize_process_details")}
        aria-orientation="vertical"
        aria-valuemin={260}
        aria-valuemax={maximum}
        aria-valuenow={actualWidth}
        tabIndex={0}
        className="panel-resizer"
        onDoubleClick={() => setWidth(340)}
        onPointerDown={(event) => {
          if (event.button !== 0) return;
          event.preventDefault();
          drag.current = { x: event.clientX, width: actualWidth };
          event.currentTarget.setPointerCapture(event.pointerId);
        }}
        onPointerMove={(event) => {
          if (drag.current)
            resize(drag.current.width + drag.current.x - event.clientX);
        }}
        onPointerUp={(event) => {
          drag.current = null;
          if (event.currentTarget.hasPointerCapture(event.pointerId))
            event.currentTarget.releasePointerCapture(event.pointerId);
        }}
        onLostPointerCapture={() => {
          drag.current = null;
        }}
        onKeyDown={(event) => {
          if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
            event.preventDefault();
            resize(actualWidth + (event.key === "ArrowLeft" ? 20 : -20));
          }
          if (event.key === "Home") {
            event.preventDefault();
            resize(260);
          }
          if (event.key === "End") {
            event.preventDefault();
            resize(maximum);
          }
        }}
      />
      <aside
        className="inspector"
        aria-label={t("inspector.k_process_details")}
      >
        <header>
          <span className="eyebrow">
            {t("inspector.k_process_details_2d6f7b50")}
          </span>
          <button
            className="icon-button"
            aria-label={t("inspector.k_close_process_details")}
            onClick={close}
          >
            <X size={16} />
          </button>
        </header>
        {availability === "missing" && (
          <p className="hint detail-refresh-note" role="status">
            {t("inspection.k_captured_process_missing")}
          </p>
        )}
        <div className="detail-title">
          <div className="detail-icon">
            <Files size={22} />
          </div>
          <h2>{details.process.name || "(unnamed)"}</h2>
          <p className="mono muted">PID {details.process.pid}</p>
        </div>
        <div
          className="detail-navigation"
          aria-label={t("inspection.k_process_inspection_views")}
        >
          <button onClick={handles}>
            <Files size={14} /> {t("inspector.k_matching_handles")}{" "}
            <span>{details.process.usages}</span>
            <ChevronRight size={14} />
          </button>
          <button onClick={ports}>
            {t("inspection.k_local_ports")} <span>{details.ports}</span>
            <ChevronRight size={14} />
          </button>
        </div>
        <section>
          <h4>{t("inspector.k_process_ancestry")}</h4>
          <p className="hint">
            {t("inspector.k_oldest_captured_parent_current_process_79d8937a")}
          </p>
          <ol
            className="ancestry"
            aria-label={t("inspector.k_process_ancestry")}
          >
            {nodes.map((node, index) => {
              const current = node.key === details.process.process_key;
              return (
                <li
                  key={node.key}
                  style={{ marginLeft: Math.min(index, 5) * 10 }}
                >
                  {index > 0 && (
                    <CornerDownRight
                      size={14}
                      className="ancestry-connector"
                      aria-hidden="true"
                    />
                  )}
                  <button
                    aria-pressed={ancestorKey === node.key}
                    className={current ? "current-process" : ""}
                    onClick={() => setAncestorKey(node.key)}
                  >
                    <span className="ancestry-name">
                      {node.name || t("common.k_unavailable")}
                      {current && <small>{t("app.k_current_process")}</small>}
                    </span>
                    <code>{node.pid}</code>
                    <ChevronRight size={14} aria-hidden="true" />
                  </button>
                </li>
              );
            })}
          </ol>
          {!details.ancestors.length && (
            <p className="hint">
              {t("inspector.k_no_parent_information_available")}
            </p>
          )}
          {ancestor && (
            <div className="ancestor-actions">
              <p className="hint">
                {t("selection.k_selected_9a976fc2")}:{" "}
                <strong>{ancestor.name || t("common.k_unavailable")}</strong> ·
                PID {ancestor.pid}
              </p>
              {!currentSelected && (
                <p className="hint">
                  {t(
                    "termination.k_stopping_a_parent_may_close_its_childre_04c26acd",
                  )}
                </p>
              )}
              {ancestor.actionable ? (
                <div className="inline-actions">
                  <button
                    disabled={!current}
                    onClick={() =>
                      currentSelected
                        ? terminateCurrent(false)
                        : terminateAncestor(ancestor.key, false)
                    }
                  >
                    {currentSelected
                      ? t("termination.k_terminate_process")
                      : t("termination.k_terminate_parent")}
                  </button>
                  <button
                    className="danger-text"
                    disabled={!current}
                    onClick={() =>
                      currentSelected
                        ? terminateCurrent(true)
                        : terminateAncestor(ancestor.key, true)
                    }
                  >
                    {currentSelected
                      ? t("termination.k_force_terminate_process")
                      : t("termination.k_force_terminate_parent")}
                  </button>
                </div>
              ) : (
                <p className="hint">
                  {t("termination.k_protected_process_or_unavailable_identity")}
                </p>
              )}
            </div>
          )}
        </section>
        <div className="detail-metrics">
          <div>
            <span>{t("inspector.k_memory")}</span>
            <strong>{memory(details.process.memory)}</strong>
          </div>
          <div>
            <span>{t("inspector.k_cpu_total_capacity")}</span>
            <strong>
              {details.process.cpu === null
                ? t("common.k_unavailable_2c9c1f79")
                : `${details.process.cpu.toFixed(1)}%`}
            </strong>
          </div>
        </div>
        <section>
          <h4>{t("inspector.k_account")}</h4>
          <p>{details.process.user}</p>
          <div className="inline-actions">
            <button disabled={!current} onClick={() => copy("pid")}>
              <Copy size={13} />
              {t("selection.k_copy_pid")}
            </button>
            <button disabled={!current} onClick={() => copy("name")}>
              <Copy size={13} />
              {t("selection.k_copy_name")}
            </button>
          </div>
        </section>
        {row && !rowCurrent && (
          <p className="hint" role="status">
            {t("inspection.k_captured_observation")}
          </p>
        )}
        {row &&
          (row.port ? (
            <section>
              <h4>{t("inspector.k_selected_port")}</h4>
              <p className="mono detail-path">{row.port.endpoint}</p>
              <p>
                {row.port.protocol} · {tValue(row.port.state)}
              </p>
            </section>
          ) : (
            <>
              {path(
                "inspector.k_selected_file_full_path",
                {
                  display: row.path,
                  reference: row.path_ref,
                },
                current && rowCurrent,
              )}
              <section>
                <h4>{t("inspector.k_file_observation")}</h4>
                <p>
                  {tValue(row.relation)} · {tValue(row.access)}
                </p>
                {row.evidence_label && <p>{tValue(row.evidence_label)}</p>}
                {row.evidence && <p className="detail-path">{row.evidence}</p>}
                {row.deleted && (
                  <p className="danger-text">{t("table.k_file_was_deleted")}</p>
                )}
              </section>
            </>
          ))}
        {path("inspector.k_executable", details.executable)}
        {path("inspector.k_working_directory", details.cwd)}
        {details.can_inspect_folder && (
          <section>
            <button disabled={!current} onClick={inspectFolder}>
              {t("inspector.k_inspect_owner_folder")}
            </button>
          </section>
        )}
        <p className="hint detail-footnote">
          {t(
            availability === "updating"
              ? "inspection.k_updating_details"
              : "inspector.k_matching_handles_are_target_observation_13c2ef67",
          )}
        </p>
      </aside>
    </div>
  );
}

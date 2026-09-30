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
import { t } from "./i18n";
export function Inspector({
  details,
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
  const path = (label: string, value: PathValue) => (
    <section>
      <h4>{t(label)}</h4>
      <p className="detail-path mono">{value.display || t("Unavailable")}</p>
      {value.display && (
        <div className="inline-actions">
          <button onClick={() => copy("path", value.reference)}>
            <Copy size={13} />
            {t("Copy path")}
          </button>
          <button onClick={() => reveal(value.reference)}>
            <ExternalLink size={13} />
            {t("Reveal")}
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
        aria-label={t("Resize process details")}
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
      <aside className="inspector" aria-label={t("Process details")}>
        <header>
          <span className="eyebrow">{t("PROCESS DETAILS")}</span>
          <button
            className="icon-button"
            aria-label={t("Close process details")}
            onClick={close}
          >
            <X size={16} />
          </button>
        </header>
        <div className="detail-title">
          <div className="detail-icon">
            <Files size={22} />
          </div>
          <h2>{details.process.name || "(unnamed)"}</h2>
          <p className="mono muted">PID {details.process.pid}</p>
        </div>
        <div
          className="detail-navigation"
          aria-label={t("Process inspection views")}
        >
          <button onClick={handles}>
            <Files size={14} /> {t("Matching handles")}{" "}
            <span>{details.process.usages}</span>
            <ChevronRight size={14} />
          </button>
          <button onClick={ports}>
            {t("Local ports")} <span>{details.ports}</span>
            <ChevronRight size={14} />
          </button>
        </div>
        <section>
          <h4>{t("Process ancestry")}</h4>
          <p className="hint">
            {t(
              "Oldest captured parent → current process. Click a row to select its actions.",
            )}
          </p>
          <ol className="ancestry" aria-label={t("Process ancestry")}>
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
                      {node.name || t("(unavailable)")}
                      {current && <small>{t("Current process")}</small>}
                    </span>
                    <code>{node.pid}</code>
                    <ChevronRight size={14} aria-hidden="true" />
                  </button>
                </li>
              );
            })}
          </ol>
          {!details.ancestors.length && (
            <p className="hint">{t("No parent information available.")}</p>
          )}
          {ancestor && (
            <div className="ancestor-actions">
              <p className="hint">
                {t("Selected")}:{" "}
                <strong>{ancestor.name || t("(unavailable)")}</strong> · PID{" "}
                {ancestor.pid}
              </p>
              {!currentSelected && (
                <p className="hint">
                  {t(
                    "Stopping a parent may close its children or your session.",
                  )}
                </p>
              )}
              {ancestor.actionable ? (
                <div className="inline-actions">
                  <button
                    onClick={() =>
                      currentSelected
                        ? terminateCurrent(false)
                        : terminateAncestor(ancestor.key, false)
                    }
                  >
                    {currentSelected
                      ? t("Terminate process…")
                      : t("Terminate parent…")}
                  </button>
                  <button
                    className="danger-text"
                    onClick={() =>
                      currentSelected
                        ? terminateCurrent(true)
                        : terminateAncestor(ancestor.key, true)
                    }
                  >
                    {currentSelected
                      ? t("Force terminate process…")
                      : t("Force terminate parent…")}
                  </button>
                </div>
              ) : (
                <p className="hint">
                  {t("Protected process or unavailable identity.")}
                </p>
              )}
            </div>
          )}
        </section>
        <div className="detail-metrics">
          <div>
            <span>{t("Memory")}</span>
            <strong>{memory(details.process.memory)}</strong>
          </div>
          <div>
            <span>{t("CPU · total capacity")}</span>
            <strong>
              {details.process.cpu === null
                ? t("Unavailable")
                : `${details.process.cpu.toFixed(1)}%`}
            </strong>
          </div>
        </div>
        <section>
          <h4>{t("Account")}</h4>
          <p>{details.process.user}</p>
          <div className="inline-actions">
            <button onClick={() => copy("pid")}>
              <Copy size={13} />
              {t("Copy PID")}
            </button>
            <button onClick={() => copy("name")}>
              <Copy size={13} />
              {t("Copy name")}
            </button>
          </div>
        </section>
        {row &&
          (row.port ? (
            <section>
              <h4>{t("Selected port")}</h4>
              <p className="mono detail-path">{row.port.endpoint}</p>
              <p>
                {row.port.protocol} · {row.port.state}
              </p>
            </section>
          ) : (
            <>
              {path("Selected file · full path", {
                display: row.path,
                reference: row.path_ref,
              })}
              <section>
                <h4>File observation</h4>
                <p>
                  {t(row.relation)} · {t(row.access)}
                </p>
                {row.evidence_label && <p>{t(row.evidence_label)}</p>}
                {row.evidence && <p className="detail-path">{row.evidence}</p>}
                {row.deleted && (
                  <p className="danger-text">{t("File was deleted")}</p>
                )}
              </section>
            </>
          ))}
        {path("Executable", details.executable)}
        {path("Working directory", details.cwd)}
        {details.can_inspect_folder && (
          <section>
            <button onClick={inspectFolder}>{t("Inspect owner folder")}</button>
          </section>
        )}
        <p className="hint detail-footnote">
          {t(
            "Matching handles are target observations, not all handles of this process. Local ports include TCP listeners and UDP bindings.",
          )}
        </p>
      </aside>
    </div>
  );
}

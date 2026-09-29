import { useEffect, useState } from "react";
import type { ColumnFilters } from "./api";
export function ColumnFilterPanel({
  value,
  ports,
  apply,
  close,
  canClose,
}: {
  value: ColumnFilters;
  ports: boolean;
  apply: (value: ColumnFilters) => void;
  close: () => void;
  canClose: boolean;
}) {
  const [draft, setDraft] = useState(value);
  useEffect(() => setDraft(value), [value]);
  const update = (
    key: keyof ColumnFilters,
    value: string | number | undefined,
  ) => setDraft((current) => ({ ...current, [key]: value }));
  const text = (key: "name" | "path" | "access", label: string) => (
    <label>
      {label}
      <input
        value={draft[key] || ""}
        onChange={(event) => update(key, event.target.value)}
        placeholder="Any · wildcards supported"
      />
    </label>
  );
  const number = (
    key: "pid" | "cpu_min" | "cpu_max" | "memory_min" | "memory_max",
    label: string,
  ) => (
    <label>
      {label}
      <input
        type="number"
        min="0"
        step={key === "pid" ? "1" : "any"}
        value={draft[key] ?? ""}
        onChange={(event) =>
          update(
            key,
            event.target.value === "" ? undefined : event.target.valueAsNumber,
          )
        }
        placeholder="Any"
      />
    </label>
  );
  const invalid =
    Object.values(draft).some(
      (value) =>
        typeof value === "number" && (!Number.isFinite(value) || value < 0),
    ) ||
    (draft.pid !== undefined &&
      (!Number.isInteger(draft.pid) || draft.pid > 4294967295)) ||
    (draft.cpu_min !== undefined &&
      draft.cpu_max !== undefined &&
      draft.cpu_min > draft.cpu_max) ||
    (draft.memory_min !== undefined &&
      draft.memory_max !== undefined &&
      draft.memory_min > draft.memory_max);
  return (
    <form
      className="column-filters"
      aria-label="Column filters"
      onSubmit={(event) => {
        event.preventDefault();
        if (!invalid) apply(draft);
      }}
    >
      <div className="column-filter-fields">
        {text("name", "Process name")}
        {number("pid", "Exact PID")}
        {text("path", ports ? "Local address" : "Full path")}
        {text("access", ports ? "Protocol / state" : "Access / relation")}
        {number("cpu_min", "CPU minimum (%)")}
        {number("cpu_max", "CPU maximum (%)")}
        {number("memory_min", "Memory minimum (MiB)")}
        {number("memory_max", "Memory maximum (MiB)")}
        {!ports && (
          <label>
            Evidence
            <select
              aria-label="Evidence"
              value={draft.evidence || "any"}
              onChange={(event) => update("evidence", event.target.value)}
            >
              <option value="any">Any evidence</option>
              <option value="present">Evidence present</option>
              <option value="kernel">Kernel lock</option>
              <option value="sharing">
                Sharing conflict · owner unverified
              </option>
              <option value="none">No observed evidence</option>
            </select>
          </label>
        )}
      </div>
      <div className="filter-actions">
        <span className="hint">
          {invalid
            ? "Use valid non-negative bounds; minimum must not exceed maximum."
            : "Filters combine with search. Unknown metrics do not match numeric bounds."}
        </span>
        <button
          type="button"
          onClick={() => {
            setDraft({});
            apply({});
          }}
        >
          Clear filters
        </button>
        <button
          type="button"
          disabled={!canClose}
          title={canClose ? "Close filters" : "Clear applied filters to close"}
          onClick={close}
        >
          Close
        </button>
        <button className="primary" disabled={invalid}>
          Apply filters
        </button>
      </div>
    </form>
  );
}

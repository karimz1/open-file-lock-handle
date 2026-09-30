import { useEffect, useState } from "react";
import type { ColumnFilters } from "./api";
import { t } from "./i18n";
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
      {t(label)}
      <input
        value={draft[key] || ""}
        onChange={(event) => update(key, event.target.value)}
        placeholder={t("Any · wildcards supported")}
      />
    </label>
  );
  const number = (
    key: "pid" | "cpu_min" | "cpu_max" | "memory_min" | "memory_max",
    label: string,
  ) => (
    <label>
      {t(label)}
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
        placeholder={t("Any")}
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
      aria-label={t("Column filters")}
      onSubmit={(event) => {
        event.preventDefault();
        if (!invalid) apply(draft);
      }}
    >
      <div className="column-filter-fields">
        {text("name", t("Process name"))}
        {number("pid", t("Exact PID"))}
        {text("path", ports ? t("Local address") : t("Full path"))}
        {text("access", ports ? t("Protocol / state") : t("Access / relation"))}
        {number("cpu_min", t("CPU minimum (%)"))}
        {number("cpu_max", t("CPU maximum (%)"))}
        {number("memory_min", t("Memory minimum (MiB)"))}
        {number("memory_max", t("Memory maximum (MiB)"))}
        {!ports && (
          <label>
            {t("Evidence")}
            <select
              aria-label={t("Evidence")}
              value={draft.evidence || "any"}
              onChange={(event) => update("evidence", event.target.value)}
            >
              <option value="any">{t("Any evidence")}</option>
              <option value="present">{t("Evidence present")}</option>
              <option value="kernel">{t("Kernel lock")}</option>
              <option value="sharing">
                {t("Sharing conflict · owner unverified")}
              </option>
              <option value="none">{t("No observed evidence")}</option>
            </select>
          </label>
        )}
      </div>
      <div className="filter-actions">
        <span className="hint">
          {invalid
            ? t(
                "Use valid non-negative bounds; minimum must not exceed maximum.",
              )
            : t(
                "Filters combine with search. Unknown metrics do not match numeric bounds.",
              )}
        </span>
        <button
          type="button"
          onClick={() => {
            setDraft({});
            apply({});
          }}
        >
          {t("Clear filters")}
        </button>
        <button
          type="button"
          disabled={!canClose}
          title={t(
            canClose ? "Close filters" : "Clear applied filters to close",
          )}
          onClick={close}
        >
          {t("Close")}
        </button>
        <button className="primary" disabled={invalid}>
          {t("Apply filters")}
        </button>
      </div>
    </form>
  );
}

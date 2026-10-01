import { useEffect, useState } from "react";
import type { ColumnFilters } from "./api";
import { t, type MessageKey } from "./i18n";
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
  const text = (key: "name" | "path" | "access", label: MessageKey) => (
    <label>
      {t(label)}
      <input
        value={draft[key] || ""}
        onChange={(event) => update(key, event.target.value)}
        placeholder={t("support.k_any_wildcards_supported")}
      />
    </label>
  );
  const number = (
    key: "pid" | "cpu_min" | "cpu_max" | "memory_min" | "memory_max",
    label: MessageKey,
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
        placeholder={t("common.k_any")}
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
      aria-label={t("filters.k_column_filters")}
      onSubmit={(event) => {
        event.preventDefault();
        if (!invalid) apply(draft);
      }}
    >
      <div className="column-filter-fields">
        {text("name", "app.k_process_name")}
        {number("pid", "app.k_exact_pid")}
        {text("path", ports ? "app.k_local_address" : "app.k_full_path")}
        {text(
          "access",
          ports ? "table.k_protocol_state" : "filters.k_access_relation",
        )}
        {number("cpu_min", "filters.k_cpu_minimum")}
        {number("cpu_max", "filters.k_cpu_maximum")}
        {number("memory_min", "filters.k_memory_minimum_mib")}
        {number("memory_max", "filters.k_memory_maximum_mib")}
        {!ports && (
          <label>
            {t("filters.k_evidence")}
            <select
              aria-label={t("filters.k_evidence")}
              value={draft.evidence || "any"}
              onChange={(event) => update("evidence", event.target.value)}
            >
              <option value="any">{t("filters.k_any_evidence")}</option>
              <option value="present">{t("filters.k_evidence_present")}</option>
              <option value="kernel">{t("app.k_kernel_lock")}</option>
              <option value="sharing">
                {t("app.k_sharing_conflict_owner_unverified")}
              </option>
              <option value="none">
                {t("filters.k_no_observed_evidence")}
              </option>
            </select>
          </label>
        )}
      </div>
      <div className="filter-actions">
        <span className="hint">
          {invalid
            ? t("filters.k_use_valid_non_negative_bounds_minimum_m_0eb65520")
            : t("filters.k_filters_combine_with_search_unknown_met_4d1f7e8f")}
        </span>
        <button
          type="button"
          onClick={() => {
            setDraft({});
            apply({});
          }}
        >
          {t("filters.k_clear_filters")}
        </button>
        <button
          type="button"
          disabled={!canClose}
          title={t(
            canClose
              ? "filters.k_close_filters"
              : "filters.k_clear_applied_filters_to_close",
          )}
          onClick={close}
        >
          {t("common.k_close")}
        </button>
        <button className="primary" disabled={invalid}>
          {t("filters.k_apply_filters")}
        </button>
      </div>
    </form>
  );
}

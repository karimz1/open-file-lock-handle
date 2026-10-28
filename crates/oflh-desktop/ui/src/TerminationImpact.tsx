import type { CappedList, ConfirmationTarget } from "./api";
import { t } from "./i18n";
import { impactSummary, listTotal } from "./termination";

function ImpactList({ heading, list }: { heading: string; list: CappedList }) {
  if (!listTotal(list)) return null;
  return (
    <div className="impact-group">
      <h4>{heading}</h4>
      <ul>
        {list.items.map((item, index) => (
          <li key={index}>{item}</li>
        ))}
        {list.more > 0 && (
          <li className="impact-more">
            {t("termination.k_impact_more", { count: list.more })}
          </li>
        )}
      </ul>
    </div>
  );
}

/** One confirmation row: name and PID, then a single impact line and optional details. */
export function TerminationTarget({ target }: { target: ConfirmationTarget }) {
  const parts = impactSummary(target);
  const impact = target.impact;
  return (
    <li>
      <div className="confirmation-target">
        <strong>
          {target.name || t("app.k_unnamed")}
          {target.role === "ancestor" && (
            <span className="role-badge">{t("termination.k_role_parent")}</span>
          )}
        </strong>
        <code>PID {target.pid}</code>
      </div>
      {impact && parts.length > 0 && (
        <>
          <p className="impact-summary">
            {parts.map((part, index) => (
              <span key={part.key}>
                {index > 0 && " · "}
                <span className={part.warn ? "impact-warning" : undefined}>
                  {t(part.key, { count: part.count })}
                </span>
              </span>
            ))}
          </p>
          <details className="impact-details">
            <summary>{t("termination.k_impact_details")}</summary>
            {impact.windows ? (
              <ImpactList
                heading={t("termination.k_impact_windows_heading")}
                list={impact.windows}
              />
            ) : (
              <p className="hint">
                {t("termination.k_impact_windows_unavailable")}
              </p>
            )}
            {impact.children && listTotal(impact.children) > 0 && (
              <>
                <ImpactList
                  heading={t("termination.k_impact_children_heading")}
                  list={impact.children}
                />
                <p className="hint">
                  {t("termination.k_impact_children_note")}
                </p>
              </>
            )}
            <ImpactList
              heading={t("termination.k_impact_files_heading")}
              list={impact.files}
            />
            <ImpactList
              heading={t("termination.k_impact_ports_heading")}
              list={impact.ports}
            />
            <p className="hint">{t("termination.k_impact_latest_scan")}</p>
          </details>
        </>
      )}
    </li>
  );
}

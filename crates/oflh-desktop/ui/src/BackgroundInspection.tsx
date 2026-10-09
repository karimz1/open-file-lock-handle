import { useEffect, useState } from "react";
import { Info } from "lucide-react";
import { type Status } from "./api";
import { t, type MessageKey } from "./i18n";
import { useInspectionProgress } from "./useInspectionProgress";
const phases: Record<string, MessageKey> = {
  processes: "inspection.k_checking_process_references",
  files: "inspection.k_checking_files",
  ports: "inspection.k_checking_ports",
  indexing: "inspection.k_preparing_results",
};
export function BackgroundInspection({
  status,
  starting,
  complete,
  automatic = false,
}: {
  status: Status;
  starting: boolean;
  complete: (status: Status) => void;
  automatic?: boolean;
}) {
  const { live, error, cancelling, cancel } = useInspectionProgress(
    status,
    starting,
    complete,
  );
  const [extended, setExtended] = useState(automatic);
  useEffect(() => {
    if (automatic) {
      setExtended(true);
      return;
    }
    const timer = setTimeout(() => setExtended(true), 250);
    return () => clearTimeout(timer);
  }, [automatic]);
  return (
    <span
      className="background-inspection"
      role="region"
      aria-label={t("inspection.k_progress")}
    >
      <span
        className="status-current"
        title={error || t("inspection.k_reload_paused")}
      >
        <span className="status-current-label" role="status">
          {t("inspection.k_updating_results")}
        </span>
        <span className="mono background-elapsed" aria-live="off">
          {((live.elapsed_ms ?? 0) / 1000).toFixed(1)} s
        </span>
      </span>
      {extended && (
        <>
          <details className="inspection-details">
            <summary
              aria-label={t("inspection.k_show_progress")}
              title={t("inspection.k_show_progress")}
            >
              <Info size={13} />
            </summary>
            <div className="inspection-details-panel">
              <strong>
                {t(
                  phases[live.progress?.phase] ??
                    "inspection.k_checking_process_references",
                )}
              </strong>
              <div
                className="inspection-progress"
                role="progressbar"
                aria-label={t("inspection.k_scanning")}
              />
              <div className="inspection-counts">
                {!!live.progress?.files && (
                  <span>
                    {t("inspection.k_files_checked", {
                      count: live.progress.files,
                    })}
                  </span>
                )}
                <span>
                  {t("inspection.k_references_checked", {
                    count: live.progress?.resources ?? 0,
                  })}
                </span>
                <span>
                  {t("inspection.k_processes_checked", {
                    count: live.progress?.processes ?? 0,
                  })}
                </span>
              </div>
              <p>{t("inspection.k_unknown_total")}</p>
            </div>
          </details>
          <button
            disabled={starting || cancelling}
            onClick={() => void cancel()}
            aria-label={
              automatic ? t("inspection.k_cancel_refresh") : undefined
            }
            title={t("inspection.k_cancel_inspection")}
          >
            {t("common.k_cancel")}
          </button>
        </>
      )}
      {error && (
        <span className="background-error" role="alert">
          {error}
        </span>
      )}
    </span>
  );
}

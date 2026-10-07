import { LoaderCircle } from "lucide-react";
import { type Status } from "./api";
import { t } from "./i18n";
import { useInspectionProgress } from "./useInspectionProgress";

export function BackgroundInspection({
  status,
  starting,
  complete,
}: {
  status: Status;
  starting: boolean;
  complete: (status: Status) => void;
}) {
  const { live, error, cancelling, cancel } = useInspectionProgress(
    status,
    starting,
    complete,
  );
  return (
    <span className="background-inspection">
      <span
        className="status-current"
        title={error || t("inspection.k_background_hint")}
      >
        <LoaderCircle size={13} className="spin" aria-hidden="true" />
        <span className="status-current-label" role="status">
          {t("inspection.k_updating_results")}
        </span>
        <span className="mono background-elapsed" aria-live="off">
          {((live.elapsed_ms ?? 0) / 1000).toFixed(1)} s
        </span>
      </span>
      <button
        disabled={starting || cancelling}
        onClick={() => void cancel()}
        aria-label={t("inspection.k_cancel_refresh")}
      >
        {t("common.k_cancel")}
      </button>
      {error && (
        <span className="background-error" role="alert">
          {error}
        </span>
      )}
    </span>
  );
}

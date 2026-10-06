import { useEffect, useState } from "react";
import { api, errorMessage, type Status } from "./api";
import { acceptStatus } from "./state";
import { t } from "./i18n";

// Observe only active work, sequentially. No progress timer survives completion.
export function useInspectionProgress(
  status: Status,
  starting: boolean,
  complete: (status: Status) => void,
) {
  const [live, setLive] = useState(status);
  const [error, setError] = useState("");
  const [cancelling, setCancelling] = useState(false);
  useEffect(() => {
    setLive(status);
    if (!status.scanning) return;
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    // One request at a time, only while scanning. Progress updates are local to
    // this component so the grid does not rerender on every polling tick.
    const poll = async () => {
      try {
        const incoming = await api.status();
        if (!active) return;
        if (incoming.generation >= status.generation) {
          setLive((current) => acceptStatus(current, incoming));
          setError("");
          if (!incoming.scanning) {
            complete(incoming);
            return;
          }
        }
      } catch {
        if (active) setError(t("inspection.k_progress_unavailable"));
      }
      if (active) timer = setTimeout(poll, 250);
    };
    timer = setTimeout(poll, 250);
    return () => {
      active = false;
      clearTimeout(timer);
    };
  }, [status.generation, status.scanning, complete]);
  const cancel = async () => {
    if (starting || cancelling) return;
    setCancelling(true);
    try {
      complete(await api.cancel());
    } catch (failure) {
      setError(errorMessage(failure));
      setCancelling(false);
    }
  };
  return { live, error, cancelling, cancel };
}

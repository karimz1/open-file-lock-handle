import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "./api";
import { checkForUpdate, installUpdate, type AvailableUpdate } from "./updater";

export type UpdateState =
  | { phase: "checking" | "current" | "check-failed" | "restart-required" }
  | {
      phase: "available" | "installing" | "install-failed";
      update: AvailableUpdate;
    };

/** One app-lifetime update result shared by navigation and Settings. */
export function useUpdates() {
  const [state, setState] = useState<UpdateState>({ phase: "checking" });
  const started = useRef(false);
  const busy = useRef(false);
  const available = useRef<AvailableUpdate | null>(null);
  const runCheck = useCallback(async () => {
    if (busy.current) return;
    busy.current = true;
    setState({ phase: "checking" });
    try {
      if (available.current?.kind === "install")
        await available.current.resource.close();
      available.current = null;
      const update = await checkForUpdate();
      available.current = update;
      setState(update ? { phase: "available", update } : { phase: "current" });
    } catch {
      setState({ phase: "check-failed" });
    } finally {
      busy.current = false;
    }
  }, []);
  useEffect(() => {
    // StrictMode replays effects; a startup check must still run only once.
    if (!started.current) {
      started.current = true;
      void runCheck();
    }
  }, [runCheck]);
  const activate = useCallback(async () => {
    const update = available.current;
    if (!update || busy.current) return;
    busy.current = true;
    try {
      if (update.kind === "download") {
        await api.openDownload();
      } else {
        setState({ phase: "installing", update });
        await installUpdate(update.resource);
        setState({ phase: "restart-required" });
      }
    } catch {
      setState({ phase: "install-failed", update });
    } finally {
      busy.current = false;
    }
  }, []);
  return { state, runCheck, activate };
}
export type Updates = ReturnType<typeof useUpdates>;

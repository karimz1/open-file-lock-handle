import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "./api";
import { checkForUpdate, installUpdate, type AvailableUpdate } from "./updater";

export type UpdateState =
  | { phase: "checking" | "current" | "check-failed" | "restart-required" }
  | {
      phase: "available" | "installing" | "install-failed";
      update: AvailableUpdate;
    };

/** One app-lifetime update result shared by navigation and Settings.
 * Startup and hourly checks only update the badge; manual checks show feedback. */
export function useUpdates() {
  const [state, setState] = useState<UpdateState>({ phase: "checking" });
  const [dialogOpen, setDialogOpen] = useState(false);
  const [notice, setNotice] = useState<"current" | "check-failed" | null>(null);
  const [checking, setChecking] = useState(false);
  const reviewing = useRef(false);
  const openDialog = useCallback(() => {
    reviewing.current = true;
    setDialogOpen(true);
  }, []);
  const closeDialog = useCallback(() => {
    reviewing.current = false;
    setDialogOpen(false);
  }, []);
  const started = useRef(false);
  const busy = useRef(false);
  const available = useRef<AvailableUpdate | null>(null);
  const runCheck = useCallback(
    async (manual = true) => {
      if (busy.current || reviewing.current) return;
      if (manual) setNotice(null);
      busy.current = true;
      setChecking(true);
      const previous = available.current;
      if (manual || !previous) setState({ phase: "checking" });
      try {
        const update = await checkForUpdate();
        if (previous?.kind === "install") await previous.resource.close();
        available.current = update;
        setState(
          update ? { phase: "available", update } : { phase: "current" },
        );
        if (manual) {
          if (update) openDialog();
          else setNotice("current");
        }
      } catch {
        // A transient network failure must not erase a known update or its resource.
        setState(
          previous
            ? { phase: "available", update: previous }
            : { phase: "check-failed" },
        );
        if (manual) setNotice("check-failed");
      } finally {
        busy.current = false;
        setChecking(false);
      }
    },
    [openDialog],
  );
  useEffect(() => {
    // StrictMode replays effects; a startup check must still run only once.
    if (!started.current) {
      started.current = true;
      void runCheck(false);
    }
  }, [runCheck]);
  useEffect(() => {
    // Do no idle rendering or polling beyond one request per hour. Never replace
    // an installer while the user is reviewing it or an installation is running.
    if (dialogOpen || state.phase === "restart-required") return;
    const timer = setInterval(() => void runCheck(false), 60 * 60 * 1000);
    return () => clearInterval(timer);
  }, [dialogOpen, state.phase, runCheck]);
  const activate = useCallback(async () => {
    const update = available.current;
    if (!update || busy.current) return;
    busy.current = true;
    try {
      if (update.kind === "download") {
        await api.openDownload();
        closeDialog();
      } else {
        setState({ phase: "installing", update });
        await installUpdate(update.resource);
        setState({ phase: "restart-required" });
        closeDialog();
      }
    } catch {
      setState({ phase: "install-failed", update });
    } finally {
      busy.current = false;
    }
  }, [closeDialog]);
  const clearNotice = useCallback(() => setNotice(null), []);
  const requestUpdate = () => {
    if (available.current && !busy.current) openDialog();
  };
  return {
    state,
    runCheck,
    activate,
    requestUpdate,
    dialogOpen,
    closeDialog,
    checking,
    notice,
    clearNotice,
  };
}
export type Updates = ReturnType<typeof useUpdates>;

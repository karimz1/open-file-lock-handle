import { useEffect, useState } from "react";
import { api } from "./api";

/** Loads one icon; resolves to a data URL, or null for the neutral fallback. */
export type IconLoader = (
  revision: number,
  reference: string,
) => Promise<string | null>;

/**
 * Per-window icon cache. Each executable reference is requested once (the
 * Rust side caches per path and does the work off the UI thread); failures
 * fall back to the neutral icon instead of retrying on every render. The
 * last icon seen per process lets the termination dialog show the same icon
 * as the table row.
 */
export function createIconCache(load: IconLoader, limit = 2000) {
  const requests = new Map<string, Promise<string | null>>();
  const done = new Map<string, string | null>();
  const byProcess = new Map<string, string>();
  const listeners = new Set<() => void>();
  const remember = <K, V>(map: Map<K, V>, key: K, value: V) => {
    if (map.size >= limit) map.delete(map.keys().next().value as K);
    map.set(key, value);
  };
  return {
    /** The icon if it is already known; undefined while unknown. */
    peek(reference: string): string | null | undefined {
      return done.get(reference);
    },
    /** Request (once) and cache the icon for a row's executable. */
    get(
      revision: number,
      reference: string,
      processKey?: string,
    ): Promise<string | null> {
      if (!reference) return Promise.resolve(null);
      const cached = requests.get(reference);
      if (cached) return cached;
      const request = load(revision, reference)
        .catch(() => null)
        .then((url) => {
          const icon = typeof url === "string" && url ? url : null;
          remember(done, reference, icon);
          if (icon && processKey) {
            remember(byProcess, processKey, icon);
            listeners.forEach((listener) => listener());
          }
          return icon;
        });
      remember(requests, reference, request);
      return request;
    },
    /** The last icon loaded for a captured process lifetime key. */
    forProcess(processKey: string): string | null {
      return byProcess.get(processKey) ?? null;
    },
    subscribe(listener: () => void) {
      listeners.add(listener);
      return () => void listeners.delete(listener);
    },
  };
}

export const processIcons = createIconCache((revision, reference) =>
  api.processIcon(revision, reference),
);

/** Lazily loads a row icon once the row is rendered (the grid is virtual). */
export function useRowIcon(
  revision: number | undefined,
  reference: string | undefined,
  processKey?: string,
): string | null {
  const [icon, setIcon] = useState(() =>
    reference ? (processIcons.peek(reference) ?? null) : null,
  );
  useEffect(() => {
    if (!reference || revision === undefined) return setIcon(null);
    const known = processIcons.peek(reference);
    if (known !== undefined) return setIcon(known);
    let live = true;
    void processIcons.get(revision, reference, processKey).then((url) => {
      if (live) setIcon(url);
    });
    return () => {
      live = false;
    };
  }, [revision, reference, processKey]);
  return icon;
}

/** The icon already loaded for a process, updating when one arrives. */
export function useProcessIcon(processKey: string): string | null {
  const [icon, setIcon] = useState(() => processIcons.forProcess(processKey));
  useEffect(() => {
    setIcon(processIcons.forProcess(processKey));
    return processIcons.subscribe(() =>
      setIcon(processIcons.forProcess(processKey)),
    );
  }, [processKey]);
  return icon;
}

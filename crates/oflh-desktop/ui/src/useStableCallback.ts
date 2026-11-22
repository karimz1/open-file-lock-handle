import { useCallback, useInsertionEffect, useRef } from "react";

/**
 * Returns a function whose identity never changes and that always calls the
 * latest `callback`. Passing it to a memoized child keeps the child from
 * re-rendering just because the parent created a new closure.
 *
 * The latest callback is stored in an insertion effect, which runs before any
 * layout effect or event handler of the same commit, so a child never calls a
 * stale one.
 */
export function useStableCallback<Args extends unknown[], Result>(
  callback: (...args: Args) => Result,
): (...args: Args) => Result {
  const latest = useRef(callback);
  useInsertionEffect(() => {
    latest.current = callback;
  });
  return useCallback((...args: Args) => latest.current(...args), []);
}

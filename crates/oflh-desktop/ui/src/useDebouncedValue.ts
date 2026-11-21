import { useEffect, useState } from "react";

/**
 * Returns `value` once it has stopped changing for `delay` ms. A delay of 0
 * passes the value through immediately, e.g. when a search is cleared.
 */
export function useDebouncedValue<T>(value: T, delay: number): T {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    if (delay <= 0) {
      setDebounced(value);
      return;
    }
    const timer = setTimeout(() => setDebounced(value), delay);
    return () => clearTimeout(timer);
  }, [value, delay]);
  return delay <= 0 ? value : debounced;
}

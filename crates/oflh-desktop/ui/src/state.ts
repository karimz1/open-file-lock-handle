import type { Status } from "./api";
export const initialStatus: Status = {
  generation: 0,
  revision: 0,
  scanning: false,
  target: "",
  processes: 0,
  ports: 0,
  usages: 0,
  warnings: [],
  error: null,
  version: "",
};
// Completion may arrive before the command acknowledgment. Never resurrect a finished scan.
export function acceptStatus(current: Status, incoming: Status): Status {
  if (incoming.generation < current.generation) return current;
  if (
    incoming.generation === current.generation &&
    !current.scanning &&
    incoming.scanning
  )
    return current;
  return incoming;
}
export function selectKey(
  current: Set<string>,
  key: string,
  additive: boolean,
): Set<string> {
  if (!additive) return new Set([key]);
  const next = new Set(current);
  if (next.has(key)) next.delete(key);
  else next.add(key);
  return next;
}
export function memory(value: number | null): string {
  if (value === null) return "—";
  return `${(value / 1048576).toFixed(value < 10485760 ? 1 : 0)} MB`;
}

// Presentation only: native paths and action references remain untouched in Rust.
// Windows separators are recognized only for drive/UNC paths; backslashes are
// valid filename characters on Unix and must not be silently reinterpreted.
export function compactPath(path: string, target: string): string {
  const windows = /^[a-z]:[\\/]/i.test(path) || path.startsWith("\\\\");
  const normalize = (value: string) =>
    windows ? value.replaceAll("\\", "/") : value;
  const displayPath = normalize(path);
  const base = normalize(target).replace(/\/+$/, "");
  if (target && displayPath.startsWith(`${base}/`)) {
    return displayPath.slice(base.length + 1) || displayPath;
  }
  return displayPath.split("/").filter(Boolean).at(-1) || path;
}

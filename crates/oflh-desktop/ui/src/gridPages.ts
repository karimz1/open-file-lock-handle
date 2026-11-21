import type { MatchingFile } from "./api";

/** Rows of a different view have other columns, so they are never reused. */
export function pageKind(query: { ports?: boolean; handles?: boolean }) {
  return `${query.ports ? "ports" : "-"}:${query.handles ? "handles" : "-"}`;
}

interface Identified {
  total?: number;
  target: string;
  kind?: string;
  queryKey: string;
}

/**
 * The rows to keep on screen, read-only, while the page for a new query is
 * on its way. Only the previous page of the same target and view qualifies:
 * a different target or view never inherits rows, and a failed request drops
 * them so nothing outdated stays next to the error. An empty page has no
 * rows to keep.
 */
export function staleRowsFor<T extends Identified>(
  shown: T | null,
  wanted: Identified,
  failed: boolean,
): T | null {
  if (
    !shown ||
    failed ||
    !shown.total ||
    shown.queryKey === wanted.queryKey ||
    shown.target !== wanted.target ||
    shown.kind !== wanted.kind
  )
    return null;
  return shown;
}

function hash(text: string) {
  let value = 5381;
  for (let index = 0; index < text.length; index++)
    value = ((value << 5) + value + text.charCodeAt(index)) | 0;
  return (value >>> 0).toString(36);
}

/**
 * Virtualizer key (it also keys the measured row height). It leaves out the
 * query so the same process keeps its measurement and its element across a
 * refilter. Matching file paths wrap differently per query, so they are part
 * of it. `layout` carries the font size and visible columns.
 */
export function virtualRowKey(
  layout: string,
  row:
    | {
        key: string;
        matching_files?: {
          total: number;
          files: Pick<MatchingFile, "path">[];
        } | null;
      }
    | undefined,
  index: number,
) {
  if (!row) return `${layout}:${index}`;
  const matches = row.matching_files;
  const signature = matches
    ? `:${matches.total}:${hash(matches.files.map((file) => file.path.display).join("\n"))}`
    : "";
  return `${layout}:${row.key}${signature}`;
}

/**
 * Clean up a path the user typed or pasted into the target field.
 *
 * Windows Explorer's "Copy as path" wraps the path in double quotes, and
 * terminals or file managers often add single quotes or surrounding spaces.
 * Only one matching pair of outer quotes is removed so that quotes inside a
 * file name are preserved. Everything else is passed to Rust unchanged.
 */
export function normalizeTypedPath(text: string): string {
  const trimmed = text.trim();
  const quote = trimmed[0];
  if (
    trimmed.length >= 2 &&
    (quote === '"' || quote === "'") &&
    trimmed.endsWith(quote)
  )
    return trimmed.slice(1, -1).trim();
  return trimmed;
}

/** Minimum window height, in root font sizes, that keeps the drag-and-drop hint. */
export const TARGET_HINT_MIN_HEIGHT_EM = 48;

/**
 * Whether the window is tall enough for the one-line drag-and-drop hint below
 * the target field. Large font sizes and short windows hide it so the results
 * grid keeps its space; the placeholder and drop overlay still explain dropping.
 */
export function showsTargetHint(windowHeight: number, fontSize: number) {
  return windowHeight >= fontSize * TARGET_HINT_MIN_HEIGHT_EM;
}

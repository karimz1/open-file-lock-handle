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

/** Abbreviate a commit for compact UI labels, keeping a trailing `-dirty`. */
export function shortCommit(commit: string): string {
  const dirty = commit.endsWith("-dirty");
  const hash = dirty ? commit.slice(0, -"-dirty".length) : commit;
  if (!hash) return "";
  return `${hash.slice(0, 7)}${dirty ? "-dirty" : ""}`;
}

/** Format a version and optional commit as `0.9.0-rc.1 (abc1234)`. */
export function formatVersionLabel(version: string, commit: string): string {
  if (!commit) return version;
  return `${version} (${shortCommit(commit)})`;
}

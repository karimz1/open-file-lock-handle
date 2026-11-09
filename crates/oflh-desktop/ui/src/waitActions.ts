/** Portable user-entered filename component; source and folder paths stay native in Rust. */
export function validWaitName(name: string): boolean {
  return (
    !!name.trim() &&
    !/[<>:"/\\|?*\u0000-\u001f\u007f-\u009f]/.test(name) &&
    !/[. ]$/.test(name) &&
    !/^(con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)/i.test(name)
  );
}
/** Display-only path parts; never used as native action inputs. */
export function waitPathParts(path: string) {
  const windows = /^[a-z]:[\\/]/i.test(path) || path.startsWith("\\\\");
  const separator = windows && path.includes("\\") ? "\\" : "/";
  const index = windows
    ? Math.max(path.lastIndexOf("\\"), path.lastIndexOf("/"))
    : path.lastIndexOf("/");
  return {
    name: path.slice(index + 1),
    parent: path.slice(0, index + 1),
    separator,
  };
}
/** Preview only: Rust constructs the resulting path from its retained source and chosen folder. */
export function waitDestinationPreview(folder: string, source: string): string {
  const { name } = waitPathParts(source);
  const { separator } = waitPathParts(folder);
  return (
    folder.replace(separator === "\\" ? /[\\/]+$/ : /\/+$/, "") +
    separator +
    name
  );
}

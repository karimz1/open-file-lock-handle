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
export interface WaitNameOptions {
  folder?: boolean;
}
/**
 * Split a filename into a base and its last extension.
 *
 * The extension is the suffix after the last `.`, matching Explorer and Finder.
 * Compound names such as `archive.tar.gz` are therefore `archive.tar` + `.gz`,
 * not a special-cased `.tar.gz`. Names whose only `.` is the first character
 * (`.gitignore`) and folder names have no extension.
 */
export function waitNameParts(
  name: string,
  options?: WaitNameOptions,
): { base: string; extension: string } {
  if (options?.folder) return { base: name, extension: "" };
  const index = name.lastIndexOf(".");
  if (index <= 0) return { base: name, extension: "" };
  return { base: name.slice(0, index), extension: name.slice(index) };
}
/** Range that should be selected on focus: the base name, or the whole name. */
export function waitNameSelectionRange(
  name: string,
  options?: WaitNameOptions,
): [number, number] {
  return [0, waitNameParts(name, options).base.length];
}
/**
 * Name that will actually be used after a rename.
 *
 * If the original has an extension and the typed name does not, the original
 * extension is appended, except for complete dotfiles (`.gitignore`) which are
 * taken as typed. A typed extension, including a different one, is kept. Folders
 * and extensionless originals never receive an invented extension.
 */
export function resolveWaitNewName(
  originalName: string,
  typedName: string,
  options?: WaitNameOptions,
): string {
  if (options?.folder) return typedName;
  const original = waitNameParts(originalName);
  if (!original.extension) return typedName;
  if (typedName.startsWith(".") && !waitNameParts(typedName).extension)
    return typedName;
  if (!waitNameParts(typedName).extension)
    return typedName + original.extension;
  return typedName;
}
/** Display-only preview of the resolved rename result. */
export function waitRenamePreview(
  parent: string,
  originalName: string,
  typedName: string,
  options?: WaitNameOptions,
): { name: string; path: string; extensionRestored: boolean } {
  const name = resolveWaitNewName(originalName, typedName, options);
  return {
    name,
    path: parent + name,
    extensionRestored: name !== typedName,
  };
}

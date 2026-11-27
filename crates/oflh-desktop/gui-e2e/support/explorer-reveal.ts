import path from "node:path";

/**
 * Helpers for the Windows Reveal check. Explorer on fully elevated GitHub
 * runners often opens the containing folder without selecting the file
 * (#202). The suite waits a short grace period for a selection, then
 * accepts a window of that folder. These functions decide that from all
 * probes at once, so one flaky probe cannot reset the timer.
 */

/** Strip the suffix Explorer adds to folder window titles. */
export function explorerFolderFromTitle(title: string): string {
  return title.replace(/ - File Explorer$/i, "").trim();
}

function sameWinPath(left: string, right: string): boolean {
  return (
    path.win32.normalize(left).toLowerCase() ===
    path.win32.normalize(right).toLowerCase()
  );
}

/** Whether Shell.Application, window titles, or UI Automation show `folder`. */
export function explorerFolderIsOpen(
  windows: { folder: string }[],
  titles: string[],
  folder: string,
  automationFolder = false,
): boolean {
  const name = path.win32.basename(folder);
  return (
    automationFolder ||
    windows.some((window) => sameWinPath(window.folder, folder)) ||
    titles.some((title) => sameWinPath(explorerFolderFromTitle(title), name))
  );
}

export type SelectionGrace = { seenSince: number };

export type GraceDecision = "waiting" | "accept" | "not-open";

/**
 * Track how long the folder window has been continuously visible. Call this
 * once per poll with the combined result of every probe. A single probe that
 * misses must not be passed in on its own: that would reset `seenSince`.
 */
export function selectionGraceDecision(
  folderOpen: boolean,
  now: number,
  grace: SelectionGrace,
  graceMs: number,
): GraceDecision {
  if (!folderOpen) {
    grace.seenSince = 0;
    return "not-open";
  }
  if (grace.seenSince === 0) {
    grace.seenSince = now;
  }
  if (now - grace.seenSince < graceMs) return "waiting";
  grace.seenSince = 0;
  return "accept";
}

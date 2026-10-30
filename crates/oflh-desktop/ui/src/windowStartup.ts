import { getCurrentWindow } from "@tauri-apps/api/window";

/** localStorage key of the "Open window maximized" setting. */
export const OPEN_MAXIMIZED_KEY = "oflh-open-maximized";

type ReadableStorage = Pick<Storage, "getItem">;
type WritableStorage = Pick<Storage, "setItem">;

/**
 * Whether the window should open maximized. Off unless the user turned it on;
 * a missing, unknown or unreadable value keeps today's restore-size startup.
 */
export function readOpenMaximized(storage: ReadableStorage): boolean {
  try {
    return storage.getItem(OPEN_MAXIMIZED_KEY) === "1";
  } catch {
    return false;
  }
}

export function writeOpenMaximized(
  storage: WritableStorage,
  enabled: boolean,
): void {
  try {
    storage.setItem(OPEN_MAXIMIZED_KEY, enabled ? "1" : "0");
  } catch {
    // The setting still applies for this session's toggle state.
  }
}

/**
 * Maximize the window once at startup when the setting is on. It is a normal
 * maximize, so the title bar's restore button returns to the configured size.
 * Failures are ignored: the window simply keeps its restore size.
 */
export async function applyStartupWindowState(
  storage: ReadableStorage,
  maximize: () => Promise<void> = () => getCurrentWindow().maximize(),
): Promise<boolean> {
  if (!readOpenMaximized(storage)) return false;
  try {
    await maximize();
    return true;
  } catch {
    return false;
  }
}

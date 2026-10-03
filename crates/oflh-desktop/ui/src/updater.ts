// Thin wrapper over the official Tauri updater/process plugins so UI code
// stays testable without a live Tauri runtime.
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

export type { Update };

/** Checks the configured update endpoint. Returns null when already current. */
export async function checkForUpdate(): Promise<Update | null> {
  return check();
}

/** Downloads and installs an update, then relaunches the app. */
export async function installUpdate(
  update: Update,
  onProgress?: (downloaded: number, total: number | undefined) => void,
): Promise<void> {
  let downloaded = 0;
  let total: number | undefined;
  await update.downloadAndInstall((event) => {
    if (event.event === "Started") {
      total = event.data.contentLength;
      onProgress?.(0, total);
    } else if (event.event === "Progress") {
      downloaded += event.data.chunkLength;
      onProgress?.(downloaded, total);
    }
  });
  await relaunch();
}

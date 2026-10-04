// Thin wrapper over the official Tauri updater/process plugins so UI code
// stays testable without a live Tauri runtime.
import { check, type Update } from "@tauri-apps/plugin-updater";
import { api } from "./api";
import { relaunch } from "@tauri-apps/plugin-process";

export type AvailableUpdate =
  | { kind: "download"; version: string }
  | { kind: "install"; version: string; resource: Update };

/** Linux uses the common release version in the manifest without retaining an
 * installer. Every published manifest includes the Windows x86-64 entry. */
export async function checkForUpdate(): Promise<AvailableUpdate | null> {
  const mode = await api.updateMode();
  const update = await check({
    timeout: 15000,
    ...(mode === "download" ? { target: "windows-x86_64" } : {}),
  });
  if (!update) return null;
  if (mode === "download") {
    const version = update.version;
    await update.close();
    return { kind: "download", version };
  }
  return { kind: "install", version: update.version, resource: update };
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

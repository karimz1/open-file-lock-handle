import os from "node:os";

/** Short operating system name used in artifact names, summaries and per-OS expectations. */
export type OperatingSystem = "windows" | "macos" | "linux";

export const operatingSystem: OperatingSystem =
  process.platform === "win32"
    ? "windows"
    : process.platform === "darwin"
      ? "macos"
      : "linux";

/** For example `linux-x64` or `windows-arm64`, so screenshots of different runners never collide. */
export const platformLabel = `${operatingSystem}-${os.arch()}`;

/** Modifier the app uses for its shortcuts (Cmd on macOS, Ctrl elsewhere). */
export const commandKey = operatingSystem === "macos" ? "Meta" : "Control";

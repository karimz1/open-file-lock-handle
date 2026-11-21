import { spawn, spawnSync, type ChildProcess } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { browser } from "@wdio/globals";
import { stubBinDir } from "./app.ts";
import { artifactsDir, suiteRoot } from "./paths.ts";
import { operatingSystem } from "./platform.ts";

/**
 * Checks against the real operating system: what OFLH put on the clipboard
 * and what it asked the file manager to show.
 */

/** Text on the system clipboard (xclip on Linux, pbpaste on macOS, Get-Clipboard on Windows). */
export function readClipboard(): string {
  const result =
    operatingSystem === "windows"
      ? spawnSync(
          "powershell.exe",
          ["-NoProfile", "-Command", "Get-Clipboard -Raw"],
          {
            encoding: "utf8",
          },
        )
      : operatingSystem === "macos"
        ? spawnSync("pbpaste", [], { encoding: "utf8" })
        : spawnSync("xclip", ["-selection", "clipboard", "-o"], {
            encoding: "utf8",
          });
  if (result.error) throw result.error;
  return (result.stdout ?? "").replace(/\r?\n$/, "");
}

/** Wait until the clipboard holds text matching `expected`. */
export async function expectClipboard(
  expected: string | RegExp,
): Promise<void> {
  let last = "";
  await browser.waitUntil(
    () => {
      last = readClipboard();
      return typeof expected === "string"
        ? last === expected
        : expected.test(last);
    },
    {
      timeout: 10_000,
      interval: 250,
      timeoutMsg: `Clipboard holds ${JSON.stringify(last)}`,
    },
  );
}

const fileManagerLog = path.join(artifactsDir, "file-manager.log");
let recorder: ChildProcess | undefined;

/**
 * Linux only: stand-ins for the desktop file manager. A D-Bus service records
 * FileManager1 requests and an `xdg-open` stub records folder opens, both into
 * `file-manager.log`. Started once per run from the WDIO launcher.
 */
export async function startLinuxFileManagerStandIns(): Promise<void> {
  if (operatingSystem !== "linux") return;
  fs.mkdirSync(stubBinDir, { recursive: true });
  const xdgOpen = path.join(stubBinDir, "xdg-open");
  fs.writeFileSync(
    xdgOpen,
    `#!/bin/sh\nfor target in "$@"; do printf 'xdg-open %s\\n' "$target" >> ${JSON.stringify(fileManagerLog)}; done\n`,
  );
  fs.chmodSync(xdgOpen, 0o755);
  recorder = spawn("python3", [
    path.join(suiteRoot, "support", "linux-file-manager-recorder.py"),
    fileManagerLog,
  ]);
  await new Promise<void>((resolve, reject) => {
    let output = "";
    const timer = setTimeout(
      () => reject(new Error(`File manager stand-in: ${output}`)),
      15_000,
    );
    const collect = (chunk: Buffer) => {
      output += chunk.toString();
      if (output.includes("ready")) {
        clearTimeout(timer);
        resolve();
      }
    };
    recorder!.stdout!.on("data", collect);
    recorder!.stderr!.on("data", collect);
    recorder!.once("exit", (code) =>
      reject(new Error(`File manager stand-in exited ${code}: ${output}`)),
    );
  });
}

export function stopLinuxFileManagerStandIns(): void {
  recorder?.kill();
}

interface ExplorerWindow {
  folder: string;
  selected: string[];
}

function explorerWindows(): ExplorerWindow[] {
  const script = `
    $windows = @((New-Object -ComObject Shell.Application).Windows() | Where-Object { $_.Document -and $_.Document.Folder })
    $windows | ForEach-Object {
      [pscustomobject]@{
        folder = $_.Document.Folder.Self.Path
        selected = @($_.Document.SelectedItems() | ForEach-Object { $_.Path })
      }
    } | ConvertTo-Json -Depth 3 -Compress`;
  const result = spawnSync(
    "powershell.exe",
    ["-STA", "-NoProfile", "-Command", script],
    { encoding: "utf8" },
  );
  const text = (result.stdout ?? "").trim();
  lastExplorerProbe = {
    status: result.status,
    stderr: (result.stderr ?? "").trim(),
    stdout: text,
  };
  if (!text) return [];
  try {
    const parsed = JSON.parse(text) as ExplorerWindow | ExplorerWindow[];
    return Array.isArray(parsed) ? parsed : [parsed];
  } catch {
    return [];
  }
}

/** Last Shell.Application probe, included in file-manager timeout messages. */
let lastExplorerProbe:
  { status: number | null; stderr: string; stdout: string } | undefined;

function explorerWindowTitles(): string[] {
  const result = spawnSync(
    "powershell.exe",
    [
      "-STA",
      "-NoProfile",
      "-Command",
      "Get-Process | Where-Object { $_.MainWindowTitle } | ForEach-Object { $_.MainWindowTitle }",
    ],
    { encoding: "utf8" },
  );
  return (result.stdout ?? "")
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
}

function finderSelection(): string[] {
  const result = spawnSync(
    "osascript",
    [
      "-e",
      'tell application "Finder" to set picked to selection as alias list',
      "-e",
      "set output to {}",
      "-e",
      "repeat with item_ in picked",
      "-e",
      "set end of output to POSIX path of item_",
      "-e",
      "end repeat",
      "-e",
      "set AppleScript's text item delimiters to linefeed",
      "-e",
      "return output as text",
    ],
    { encoding: "utf8", timeout: 15_000 },
  );
  if (result.status !== 0) {
    throw new Error(`Could not read the Finder selection: ${result.stderr}`);
  }
  return result.stdout
    .split("\n")
    .map((line) => line.trim().replace(/\/$/, ""))
    .filter(Boolean);
}

const samePath = (left: string, right: string) =>
  operatingSystem === "windows"
    ? path.win32.normalize(left).toLowerCase() ===
      path.win32.normalize(right).toLowerCase()
    : path.normalize(left).replace(/\/$/, "") ===
      path.normalize(right).replace(/\/$/, "");

/**
 * Wait until the platform file manager shows `target` selected in its folder:
 * Explorer on Windows, Finder on macOS, the FileManager1 ShowItems request on Linux.
 */
export async function expectFileManagerToReveal(target: string): Promise<void> {
  await waitForFileManager(target, "reveal");
}

/**
 * Wait until the file manager opens `target` as a folder, not as a selected
 * item in its parent. After #191, directory handles use this path.
 */
export async function expectFileManagerToOpen(target: string): Promise<void> {
  await waitForFileManager(target, "open");
}

function linuxFileManagerLog(): string {
  return fs.existsSync(fileManagerLog)
    ? fs.readFileSync(fileManagerLog, "utf8")
    : "";
}

function finderWindows(): string[] {
  const result = spawnSync(
    "osascript",
    [
      "-e",
      'tell application "Finder"',
      "-e",
      "set output to {}",
      "-e",
      "repeat with w in windows",
      "-e",
      "try",
      "-e",
      "set end of output to POSIX path of (target of w as alias)",
      "-e",
      "end try",
      "-e",
      "end repeat",
      "-e",
      "set AppleScript's text item delimiters to linefeed",
      "-e",
      "return output as text",
      "-e",
      "end tell",
    ],
    { encoding: "utf8", timeout: 15_000 },
  );
  if (result.status !== 0) {
    throw new Error(`Could not read Finder windows: ${result.stderr}`);
  }
  return result.stdout
    .split("\n")
    .map((line) => line.trim().replace(/\/$/, ""))
    .filter(Boolean);
}

async function waitForFileManager(
  target: string,
  mode: "reveal" | "open",
): Promise<void> {
  let seen = "";
  const uri = pathToFileURL(target).href;
  await browser.waitUntil(
    () => {
      if (operatingSystem === "linux") {
        seen = linuxFileManagerLog();
        return mode === "open"
          ? seen.includes(`xdg-open ${uri}`) ||
              seen.includes(`ShowFolders ${uri}`)
          : seen.includes(`ShowItems ${uri}`);
      }
      if (operatingSystem === "windows") {
        const windows = explorerWindows();
        const titles = explorerWindowTitles();
        seen = JSON.stringify({ windows, titles, probe: lastExplorerProbe });
        if (mode === "open") {
          return (
            windows.some((window) => samePath(window.folder, target)) ||
            titles.some((title) => samePath(title, path.win32.basename(target)))
          );
        }
        return windows.some((window) =>
          window.selected.some((item) => samePath(item, target)),
        );
      }
      if (mode === "open") {
        const opened = finderWindows();
        seen = opened.join(", ");
        return opened.some((folder) => samePath(folder, target));
      }
      const selection = finderSelection();
      seen = selection.join(", ");
      return selection.some((item) => samePath(item, target));
    },
    {
      timeout: 15_000,
      interval: 500,
      timeoutMsg: `The file manager did not ${mode} ${target}. It showed: ${seen || "nothing"}`,
    },
  );
}

/** Close Explorer windows opened by reveal tests so later checks start clean (Windows only). */
export function closeFileManagerWindows(): void {
  if (operatingSystem === "windows") {
    spawnSync(
      "powershell.exe",
      [
        "-NoProfile",
        "-Command",
        "(New-Object -ComObject Shell.Application).Windows() | Where-Object { $_.Document -and $_.Document.Folder } | ForEach-Object { $_.Quit() }",
      ],
      { stdio: "ignore" },
    );
  } else if (operatingSystem === "linux" && fs.existsSync(fileManagerLog)) {
    fs.rmSync(fileManagerLog);
  }
}

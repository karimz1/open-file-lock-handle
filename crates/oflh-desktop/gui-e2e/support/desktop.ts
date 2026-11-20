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
    ["-NoProfile", "-Command", script],
    {
      encoding: "utf8",
    },
  );
  const text = (result.stdout ?? "").trim();
  if (!text) return [];
  const parsed = JSON.parse(text) as ExplorerWindow | ExplorerWindow[];
  return Array.isArray(parsed) ? parsed : [parsed];
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
 * Explorer on Windows, Finder on macOS, the FileManager1 request on Linux.
 */
export async function expectFileManagerToReveal(target: string): Promise<void> {
  let seen = "";
  await browser.waitUntil(
    () => {
      if (operatingSystem === "linux") {
        seen = fs.existsSync(fileManagerLog)
          ? fs.readFileSync(fileManagerLog, "utf8")
          : "";
        return seen.includes(`ShowItems ${pathToFileURL(target).href}`);
      }
      if (operatingSystem === "windows") {
        const windows = explorerWindows();
        seen = JSON.stringify(windows);
        return windows.some((window) =>
          window.selected.some((item) => samePath(item, target)),
        );
      }
      const selection = finderSelection();
      seen = selection.join(", ");
      return selection.some((item) => samePath(item, target));
    },
    {
      timeout: 15_000,
      interval: 500,
      timeoutMsg: `The file manager did not show ${target}. It showed: ${seen || "nothing"}`,
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

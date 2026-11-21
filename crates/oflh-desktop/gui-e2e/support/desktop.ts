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

interface AutomationExplorerWindow {
  title: string;
  selected: string[];
}

/**
 * Explorer folder windows as UI Automation sees them: window title and the
 * names of the selected items. Shell.Application only lists windows of an
 * Explorer at the caller's integrity level, so an elevated test runner can
 * get an empty list while the window is on screen. UI Automation can read
 * windows of the same or a lower integrity level.
 */
function automationExplorerWindows(): AutomationExplorerWindow[] {
  const script = `
    Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
    $A = [System.Windows.Automation.AutomationElement]
    $scope = [System.Windows.Automation.TreeScope]
    $cabinet = New-Object System.Windows.Automation.PropertyCondition($A::ClassNameProperty, 'CabinetWClass')
    $item = New-Object System.Windows.Automation.PropertyCondition($A::ControlTypeProperty, [System.Windows.Automation.ControlType]::ListItem)
    $picked = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.SelectionItemPattern]::IsSelectedProperty, $true)
    $both = New-Object System.Windows.Automation.AndCondition($item, $picked)
    @($A::RootElement.FindAll($scope::Children, $cabinet) | ForEach-Object {
      [pscustomobject]@{
        title = $_.Current.Name
        selected = @($_.FindAll($scope::Descendants, $both) | ForEach-Object { $_.Current.Name })
      }
    }) | ConvertTo-Json -Depth 3 -Compress`;
  const result = spawnSync(
    "powershell.exe",
    ["-STA", "-NoProfile", "-Command", script],
    { encoding: "utf8", timeout: 20_000 },
  );
  const text = (result.stdout ?? "").trim();
  lastAutomationProbe = {
    status: result.status,
    stderr: (result.stderr ?? "").trim().slice(0, 500),
    stdout: text.slice(0, 2_000),
  };
  if (!text) return [];
  try {
    const parsed = JSON.parse(text) as
      AutomationExplorerWindow | AutomationExplorerWindow[];
    return Array.isArray(parsed) ? parsed : [parsed];
  } catch {
    return [];
  }
}

let lastAutomationProbe:
  { status: number | null; stderr: string; stdout: string } | undefined;

/** Whether UI Automation shows `target` selected in an Explorer window of its folder. */
function automationShowsSelected(
  windows: AutomationExplorerWindow[],
  target: string,
): boolean {
  const folder = path.win32.basename(path.win32.dirname(target)).toLowerCase();
  const name = path.win32.basename(target).toLowerCase();
  // Explorer hides known extensions by default.
  const stem = path.win32.parse(target).name.toLowerCase();
  return windows.some(
    (window) =>
      window.title
        .replace(/ - File Explorer$/i, "")
        .trim()
        .toLowerCase() === folder &&
      window.selected.some((item) =>
        [name, stem].includes(item.trim().toLowerCase()),
      ),
  );
}

/** Facts that tell a missing window apart from a probe that cannot see it. */
function windowsDiagnostics(): string {
  const script = `
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $elevated = (New-Object Security.Principal.WindowsPrincipal($identity)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    $lua = (Get-ItemProperty 'HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Policies\\System' -ErrorAction SilentlyContinue).EnableLUA
    $explorer = @(Get-Process explorer -ErrorAction SilentlyContinue | ForEach-Object { "$($_.Id) session $($_.SessionId)" })
    $shellWindows = @((New-Object -ComObject Shell.Application).Windows()).Count
    "probe elevated=$elevated session=$((Get-Process -Id $PID).SessionId) EnableLUA=$lua explorer=[$($explorer -join ', ')] shellWindows=$shellWindows"`;
  const result = spawnSync(
    "powershell.exe",
    ["-STA", "-NoProfile", "-Command", script],
    { encoding: "utf8", timeout: 20_000 },
  );
  return `${(result.stdout ?? "").trim()} ${(result.stderr ?? "").trim().slice(0, 300)}`.trim();
}

/** Save the whole Windows desktop next to the WebView screenshots. */
function captureWindowsDesktop(name: string): void {
  const directory = path.join(artifactsDir, "screenshots");
  fs.mkdirSync(directory, { recursive: true });
  const file = path.join(
    directory,
    `${process.env.OFLH_E2E_SPEC ?? "spec"}-desktop-${name.replace(/[^a-z0-9]+/gi, "-").toLowerCase()}.png`,
  );
  const script = `
    Add-Type -AssemblyName System.Windows.Forms, System.Drawing
    $bounds = [System.Windows.Forms.SystemInformation]::VirtualScreen
    $bitmap = New-Object System.Drawing.Bitmap($bounds.Width, $bounds.Height)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $graphics.CopyFromScreen($bounds.Left, $bounds.Top, 0, 0, $bitmap.Size)
    $bitmap.Save('${file.replace(/'/g, "''")}', [System.Drawing.Imaging.ImageFormat]::Png)`;
  spawnSync("powershell.exe", ["-STA", "-NoProfile", "-Command", script], {
    stdio: "ignore",
    timeout: 20_000,
  });
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
  try {
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
              titles.some((title) =>
                samePath(title, path.win32.basename(target)),
              )
            );
          }
          if (
            windows.some((window) =>
              window.selected.some((item) => samePath(item, target)),
            )
          )
            return true;
          const automation = automationExplorerWindows();
          seen = JSON.stringify({
            windows,
            titles,
            probe: lastExplorerProbe,
            automation,
            automationProbe: lastAutomationProbe,
          });
          return automationShowsSelected(automation, target);
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
      { timeout: 20_000, interval: 500 },
    );
  } catch (error) {
    // The message is built after the wait: `seen` holds the last probe.
    let facts = "";
    if (operatingSystem === "windows") {
      facts = ` Diagnostics: ${windowsDiagnostics()}`;
      captureWindowsDesktop(`${mode} ${path.win32.basename(target)}`);
    }
    throw new Error(
      `The file manager did not ${mode} ${target}. It showed: ${seen || "nothing"}.${facts}`,
      { cause: error },
    );
  }
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

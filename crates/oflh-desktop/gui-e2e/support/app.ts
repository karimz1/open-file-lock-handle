import { spawn, spawnSync, type ChildProcess } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { artifactsDir, repositoryRoot } from "./paths.ts";
import { operatingSystem } from "./platform.ts";

/**
 * Port of the WebDriver server that `tauri-plugin-wdio-webdriver` embeds in
 * test builds (`--features gui-e2e`). It only listens on 127.0.0.1.
 */
export const webdriverPort = Number(
  process.env.OFLH_E2E_WEBDRIVER_PORT ?? 4445,
);

/** The desktop binary built with `--features gui-e2e`; override with OFLH_E2E_APP. */
export function appBinary(): string {
  const configured = process.env.OFLH_E2E_APP;
  if (configured) return path.resolve(configured);
  const name =
    operatingSystem === "windows" ? "oflh-desktop.exe" : "oflh-desktop";
  return path.join(repositoryRoot, "target", "debug", name);
}

/** Directory with stand-in helper programs put first on the app's PATH (Linux `xdg-open`). */
export const stubBinDir = path.join(artifactsDir, "stub-bin");

/**
 * The spec whose app starts the way the Explorer context menu starts it:
 * `oflh-desktop --inspect <folder>`.
 */
export const startupTargetSpec = "09-launch-from-file-manager";

/** The folder that spec's app is started with, created on first use. */
export function startupTarget(): string {
  const folder = path.join(artifactsDir, "startup-target");
  fs.mkdirSync(folder, { recursive: true });
  return fs.realpathSync.native(folder);
}

/** Command-line arguments for the app instance of one spec file. */
export function launchArguments(specName: string): string[] {
  return specName === startupTargetSpec ? ["--inspect", startupTarget()] : [];
}

/** True for the Windows-only portable-layout spec. */
export function isPortableSpec(specName: string): boolean {
  return operatingSystem === "windows" && specName.includes("windows-portable");
}

const bundleId = "io.github.karimz1.oflh";

export type PortableLayout = {
  root: string;
  data: string;
  beforeAppData: string[];
  beforeLocal: string[];
  beforeAppDataBundles: string[];
  beforeLocalBundles: string[];
};

const portableLayoutFile = path.join(artifactsDir, "portable-layout.json");
let portableState: PortableLayout | undefined;

function readPortableLayout(): PortableLayout | undefined {
  if (portableState) return portableState;
  if (!fs.existsSync(portableLayoutFile)) return undefined;
  portableState = JSON.parse(
    fs.readFileSync(portableLayoutFile, "utf8"),
  ) as PortableLayout;
  return portableState;
}

function writePortableLayout(layout: PortableLayout): void {
  portableState = layout;
  fs.mkdirSync(artifactsDir, { recursive: true });
  fs.writeFileSync(
    portableLayoutFile,
    `${JSON.stringify(layout, null, 2)}
`,
  );
}

function listing(directory: string): string[] {
  if (!fs.existsSync(directory)) return [];
  const names: string[] = [];
  for (const entry of fs.readdirSync(directory, { recursive: true })) {
    names.push(String(entry));
  }
  names.sort();
  return names;
}

function bundleNames(root: string): string[] {
  if (!fs.existsSync(root)) return [];
  return fs
    .readdirSync(root)
    .filter((name) => name.startsWith(bundleId))
    .sort();
}

/** Folder that holds the copied exe, `portable` marker and `data` directory. */
export function portableLayout(): PortableLayout {
  const layout = readPortableLayout();
  if (!layout) {
    throw new Error("the portable layout has not been prepared");
  }
  return layout;
}

function preparePortableBinary(binary: string): {
  binary: string;
  cwd: string;
} {
  const existing = readPortableLayout();
  if (!existing || !fs.existsSync(path.join(existing.root, "portable"))) {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "oflh-e2e-portable-"));
    fs.copyFileSync(binary, path.join(root, path.basename(binary)));
    fs.writeFileSync(path.join(root, "portable"), "");
    fs.mkdirSync(path.join(root, "data"), { recursive: true });
    const appDataRoot = process.env.APPDATA ?? "";
    const localRoot = process.env.LOCALAPPDATA ?? "";
    writePortableLayout({
      root,
      data: path.join(root, "data"),
      beforeAppData: listing(path.join(appDataRoot, bundleId)),
      beforeLocal: listing(path.join(localRoot, bundleId)),
      beforeAppDataBundles: bundleNames(appDataRoot),
      beforeLocalBundles: bundleNames(localRoot),
    });
  }
  const layout = portableLayout();
  return {
    binary: path.join(layout.root, path.basename(binary)),
    cwd: layout.root,
  };
}

export interface RunningApp {
  child: ChildProcess;
  logFile: string;
}

/**
 * Start one app instance for one spec file and wait until its WebDriver
 * server answers. Linux gets a private XDG profile per spec, so settings and
 * recent targets start empty; Windows and macOS keep the runner profile and
 * the specs reset the WebView storage themselves (see `resetWorkspace`).
 */
export async function launchApp(
  specName: string,
  args: string[] = [],
): Promise<RunningApp> {
  let binary = appBinary();
  if (!fs.existsSync(binary)) {
    throw new Error(
      `${binary} does not exist. Build it first: npm --prefix crates/oflh-desktop/ui run tauri -- build --debug --no-bundle --features gui-e2e`,
    );
  }
  const portable = isPortableSpec(specName);
  let cwd: string | undefined;
  if (portable) {
    const prepared = preparePortableBinary(binary);
    binary = prepared.binary;
    cwd = prepared.cwd;
  }
  await waitForPortToClose();
  const logsDir = path.join(artifactsDir, "app-logs");
  fs.mkdirSync(logsDir, { recursive: true });
  const logFile = path.join(logsDir, `${specName}.log`);
  const log = fs.openSync(logFile, "w");
  const env: NodeJS.ProcessEnv = {
    ...process.env,
    TAURI_WEBDRIVER_PORT: String(webdriverPort),
    PATH: `${stubBinDir}${path.delimiter}${process.env.PATH ?? ""}`,
  };
  // Isolated profiles: Linux via XDG, Windows via WebView2 user data.
  // macOS keeps the runner profile; specs reset WebView storage themselves.
  const profile = fs.mkdtempSync(
    path.join(os.tmpdir(), `oflh-e2e-profile-${specName}-`),
  );
  if (operatingSystem === "linux") {
    env.XDG_DATA_HOME = path.join(profile, "data");
    env.XDG_CONFIG_HOME = path.join(profile, "config");
    env.XDG_CACHE_HOME = path.join(profile, "cache");
  } else if (operatingSystem === "windows" && !portable) {
    // Portable mode keeps the WebView profile in data/; do not override it.
    env.WEBVIEW2_USER_DATA_FOLDER = path.join(profile, "webview2");
  }
  fs.writeSync(
    log,
    `starting ${binary} ${args.join(" ")} for ${specName} (webdriver port ${webdriverPort})\n`,
  );
  const child = spawn(binary, args, { cwd, env, stdio: ["ignore", log, log] });
  fs.writeSync(log, `spawned pid ${child.pid ?? "unknown"}\n`);
  const deadline = Date.now() + (specName === "warmup" ? 120_000 : 90_000);
  while (Date.now() < deadline) {
    if (child.exitCode !== null) {
      throw new Error(
        `The app exited with ${child.exitCode} during startup; see ${logFile}`,
      );
    }
    if (await webdriverReady()) return { child, logFile };
    await sleep(250);
  }
  const stillRunning = child.exitCode === null;
  await stopApp({ child, logFile });
  throw new Error(
    `The app's WebDriver server did not answer within 90 s (process ${stillRunning ? "was still running" : `exited with ${child.exitCode}`}); see ${logFile}`,
  );
}

/** Stop the app and everything it started (WebView2 and inspection helpers). */
export async function stopApp({ child }: RunningApp): Promise<void> {
  if (child.exitCode === null && child.pid) {
    if (operatingSystem === "windows") {
      spawnSync("taskkill", ["/PID", String(child.pid), "/T", "/F"], {
        stdio: "ignore",
      });
    } else {
      child.kill("SIGTERM");
      const exited = await waitForExit(child, 5_000);
      if (!exited) child.kill("SIGKILL");
    }
    await waitForExit(child, 10_000);
  }
  await waitForPortToClose();
}

async function webdriverReady(): Promise<boolean> {
  try {
    const response = await fetch(`http://127.0.0.1:${webdriverPort}/status`);
    const body = (await response.json()) as { value?: { ready?: boolean } };
    return body.value?.ready === true;
  } catch {
    return false;
  }
}

async function waitForPortToClose(): Promise<void> {
  const deadline = Date.now() + 15_000;
  while (Date.now() < deadline && (await webdriverReady())) await sleep(250);
  if (await webdriverReady()) {
    throw new Error(
      `Another app instance still serves WebDriver on port ${webdriverPort}`,
    );
  }
}

function waitForExit(child: ChildProcess, timeout: number): Promise<boolean> {
  if (child.exitCode !== null || child.signalCode !== null)
    return Promise.resolve(true);
  return new Promise((resolve) => {
    const timer = setTimeout(() => resolve(false), timeout);
    child.once("exit", () => {
      clearTimeout(timer);
      resolve(true);
    });
  });
}

export const sleep = (milliseconds: number) =>
  new Promise<void>((resolve) => setTimeout(resolve, milliseconds));

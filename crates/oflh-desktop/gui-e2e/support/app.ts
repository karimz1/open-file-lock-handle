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
export async function launchApp(specName: string): Promise<RunningApp> {
  const binary = appBinary();
  if (!fs.existsSync(binary)) {
    throw new Error(
      `${binary} does not exist. Build it first: npm --prefix crates/oflh-desktop/ui run tauri -- build --debug --no-bundle --features gui-e2e`,
    );
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
  if (operatingSystem === "linux") {
    const profile = fs.mkdtempSync(
      path.join(os.tmpdir(), `oflh-e2e-profile-${specName}-`),
    );
    env.XDG_DATA_HOME = path.join(profile, "data");
    env.XDG_CONFIG_HOME = path.join(profile, "config");
    env.XDG_CACHE_HOME = path.join(profile, "cache");
  }
  const child = spawn(binary, [], { env, stdio: ["ignore", log, log] });
  const deadline = Date.now() + 90_000;
  while (Date.now() < deadline) {
    if (child.exitCode !== null) {
      throw new Error(
        `The app exited with ${child.exitCode} during startup; see ${logFile}`,
      );
    }
    if (await webdriverReady()) return { child, logFile };
    await sleep(250);
  }
  await stopApp({ child, logFile });
  throw new Error(
    `The app's WebDriver server did not answer within 90 s; see ${logFile}`,
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

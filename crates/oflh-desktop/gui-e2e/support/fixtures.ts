import {
  spawn,
  spawnSync,
  type ChildProcessWithoutNullStreams,
} from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { suiteRoot } from "./paths.ts";
import { operatingSystem } from "./platform.ts";

/** A folder with a few small files, created under the system temp folder. */
export interface FixtureTree {
  /** Real path of the folder the tests inspect. */
  folder: string;
  /** Files inside `folder`, by short name. */
  file: (name: string) => string;
  /** A file next to `folder`, not inside it. */
  outside: string;
}

/**
 * Create `<tmp>/oflh-e2e-<name>-xxxx/target/{names}` plus one file outside the
 * target. Paths are resolved to their real long form (macOS `/private/var`,
 * Windows 8.3 short names), because that is what the app shows.
 */
export function createFixtureTree(
  name: string,
  fileNames: string[],
): FixtureTree {
  const root = fs.realpathSync.native(
    fs.mkdtempSync(path.join(os.tmpdir(), `oflh-e2e-${name}-`)),
  );
  const folder = path.join(root, "target");
  fs.mkdirSync(folder);
  for (const fileName of fileNames) {
    fs.writeFileSync(
      path.join(folder, fileName),
      `OFLH end-to-end fixture ${fileName}\n`,
    );
  }
  const outside = path.join(root, "outside.txt");
  fs.writeFileSync(outside, "outside the inspected folder\n");
  return { folder, outside, file: (fileName) => path.join(folder, fileName) };
}

/** A running fixture process that keeps files open. */
export interface Holder {
  pid: number;
  /** The TCP port on 127.0.0.1 when started with `listen`. */
  port?: number;
  /** Close the files but keep the process running. */
  release(): Promise<void>;
  /** Kill the process if it still runs. */
  stop(): Promise<void>;
  /** Resolves true once the process has exited. */
  waitForExit(timeout: number): Promise<boolean>;
  isRunning(): boolean;
}

const running = new Set<Holder>();

/**
 * Start a process that opens `files` and keeps them open. `windowed` (Windows
 * only) uses a WinForms window, so a normal close can reach it through WM_CLOSE.
 * `cwd` becomes the process working directory, which OFLH lists as a folder entry.
 * `listen` also binds a TCP port on 127.0.0.1 for the Ports view.
 */
export async function startHolder(
  files: string[],
  {
    cwd,
    windowed = false,
    listen = false,
  }: { cwd?: string; windowed?: boolean; listen?: boolean } = {},
): Promise<Holder> {
  if (windowed && operatingSystem !== "windows") {
    throw new Error("The windowed holder is only available on Windows");
  }
  const child: ChildProcessWithoutNullStreams = windowed
    ? spawn(
        "powershell.exe",
        [
          "-STA",
          "-NoProfile",
          "-ExecutionPolicy",
          "Bypass",
          "-File",
          path.join(suiteRoot, "support", "hold-files-windowed.ps1"),
          ...files,
        ],
        { cwd },
      )
    : spawn(
        process.execPath,
        [
          path.join(suiteRoot, "support", "hold-files.mjs"),
          ...(listen ? ["--listen"] : []),
          ...files,
        ],
        { cwd },
      );
  let output = "";
  child.stdout.setEncoding("utf8");
  child.stderr.setEncoding("utf8");
  child.stderr.on("data", (text: string) => (output += text));
  let exited = false;
  const exit = new Promise<void>((resolve) =>
    child.once("exit", () => {
      exited = true;
      resolve();
    }),
  );
  const ready = await new Promise<{ pid: number; port?: number }>(
    (resolve, reject) => {
      const timer = setTimeout(
        () => reject(new Error(`Fixture holder did not start: ${output}`)),
        30_000,
      );
      child.stdout.on("data", (text: string) => {
        output += text;
        const match = /ready (\d+)(?: port (\d+))?/.exec(output);
        if (match) {
          clearTimeout(timer);
          resolve({
            pid: Number(match[1]),
            port: match[2] ? Number(match[2]) : undefined,
          });
        }
      });
      child.once("exit", (code) => {
        clearTimeout(timer);
        reject(new Error(`Fixture holder exited with ${code}: ${output}`));
      });
    },
  );
  const waitForExit = async (timeout: number) =>
    exited ||
    (await Promise.race([
      exit.then(() => true),
      new Promise<boolean>((resolve) =>
        setTimeout(() => resolve(false), timeout),
      ),
    ]));
  const holder: Holder = {
    pid: ready.pid,
    port: ready.port,
    isRunning: () => !exited,
    waitForExit,
    async release() {
      if (windowed)
        throw new Error("The windowed holder cannot release its files");
      child.stdin.write("release\n");
      const deadline = Date.now() + 10_000;
      while (!output.includes("released") && Date.now() < deadline) {
        await new Promise((resolve) => setTimeout(resolve, 50));
      }
      if (!output.includes("released"))
        throw new Error("Fixture holder did not release");
    },
    async stop() {
      running.delete(holder);
      if (exited) return;
      if (operatingSystem === "windows") {
        spawnSync("taskkill", ["/PID", String(child.pid), "/T", "/F"], {
          stdio: "ignore",
        });
      } else {
        child.kill("SIGKILL");
      }
      await waitForExit(10_000);
    },
  };
  running.add(holder);
  return holder;
}

/** Stop every holder a spec started; called from the global `after` hook. */
export async function stopAllHolders(): Promise<void> {
  await Promise.all([...running].map((holder) => holder.stop()));
}

import fs from "node:fs";
import path from "node:path";
import {
  launchApp,
  stopApp,
  webdriverPort,
  type RunningApp,
} from "./support/app.ts";
import {
  startLinuxFileManagerStandIns,
  stopLinuxFileManagerStandIns,
} from "./support/desktop.ts";
import { stopAllHolders } from "./support/fixtures.ts";
import { artifactsDir } from "./support/paths.ts";
import {
  classify,
  knownFailures,
  recordResult,
  writeSummary,
} from "./support/results.ts";
import { screenshot } from "./support/ui.ts";

const specName = (file: string) =>
  path.basename(file).replace(/\.e2e\.ts$/, "");
const apps = new Map<string, RunningApp>();

/**
 * WebdriverIO drives the real desktop app through the W3C WebDriver server that
 * `tauri-plugin-wdio-webdriver` embeds in builds with `--features gui-e2e`.
 * Each spec file gets a fresh app process; specs run one after another because
 * they share the window, the port and the system clipboard.
 */
export const config: WebdriverIO.Config = {
  runner: "local",
  hostname: "127.0.0.1",
  port: webdriverPort,
  specs: ["./specs/*.e2e.ts"],
  maxInstances: 1,
  capabilities: [{}],
  logLevel: "warn",
  bail: 0,
  waitforTimeout: 20_000,
  connectionRetryTimeout: 120_000,
  connectionRetryCount: 2,
  framework: "mocha",
  reporters: ["spec"],
  mochaOpts: { ui: "bdd", timeout: 180_000 },

  onPrepare() {
    fs.rmSync(artifactsDir, { recursive: true, force: true });
    fs.mkdirSync(artifactsDir, { recursive: true });
    return startLinuxFileManagerStandIns();
  },

  async onWorkerStart(cid, _capabilities, specs) {
    apps.set(cid, await launchApp(specName(specs[0] ?? cid)));
  },

  async onWorkerEnd(cid) {
    const app = apps.get(cid);
    apps.delete(cid);
    if (app) await stopApp(app);
  },

  before(_capabilities, specs) {
    process.env.OFLH_E2E_SPEC = specName(specs[0] ?? "spec");
  },

  async afterTest(test, _context, { passed, error }) {
    const title = test.title;
    const outcome = classify(title, passed, false);
    if (outcome === "not applicable") return;
    await screenshot(`end of ${title}`).catch(() => undefined);
    recordResult({
      spec: process.env.OFLH_E2E_SPEC ?? "spec",
      title,
      outcome,
      detail:
        outcome === "known failing"
          ? (knownFailures.get(test.fullTitle ?? title) ??
            knownFailures.values().next().value)
          : error
            ? String(error.message ?? error).split("\n")[0]
            : undefined,
    });
  },

  async after() {
    await stopAllHolders();
  },

  onComplete() {
    stopLinuxFileManagerStandIns();
    const { markdown } = writeSummary();
    console.log(markdown);
    if (process.env.GITHUB_STEP_SUMMARY)
      fs.appendFileSync(process.env.GITHUB_STEP_SUMMARY, markdown);
  },
};

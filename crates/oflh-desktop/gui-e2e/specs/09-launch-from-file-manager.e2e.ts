import { spawn } from "node:child_process";
import { expect } from "@wdio/globals";
import { appBinary, startupTarget } from "../support/app.ts";
import { createFixtureTree, startHolder } from "../support/fixtures.ts";
import { itOn } from "../support/results.ts";
import {
  browser,
  resetWorkspace,
  screenshot,
  targetInput,
  waitForInspectionComplete,
  waitForRowWith,
} from "../support/ui.ts";

/** Wait until the target bar shows `folder`. */
async function waitForTarget(folder: string, what: string): Promise<void> {
  const input = await targetInput();
  await input.waitForDisplayed({ timeout: 20_000 });
  await browser.waitUntil(async () => (await input.getValue()) === folder, {
    timeout: 30_000,
    interval: 250,
    timeoutMsg: `${what}: the target is "${await input.getValue()}", not ${folder}`,
  });
}

/**
 * The Explorer context menu (and "Open with" on other systems) starts the app
 * as `oflh-desktop --inspect <path>`. wdio.conf.ts starts this spec's app that
 * way with `startupTarget()`.
 */
describe("Starting from the file manager", () => {
  before(async () => {
    // Storage only; the inspection started at launch lives in the backend.
    await resetWorkspace();
  });

  it("launching with --inspect opens the app with that folder inspected", async () => {
    await waitForTarget(startupTarget(), "After a launch with --inspect");
    await waitForInspectionComplete();
    await screenshot("started with a target");
  });

  itOn(
    ["windows"],
    "Only Windows hands a second launch to the open window; Linux and macOS start an independent window",
    "a second context-menu launch hands its folder to the open window and exits",
    async () => {
      const tree = createFixtureTree("second-launch", ["held.txt"]);
      const holder = await startHolder([tree.file("held.txt")]);
      const second = spawn(appBinary(), ["--inspect", tree.folder], {
        stdio: "ignore",
      });
      const exitCode = await new Promise<number | null>((resolve) => {
        const timer = setTimeout(() => resolve(null), 30_000);
        second.once("exit", (code) => {
          clearTimeout(timer);
          resolve(code);
        });
      });
      if (exitCode === null) second.kill();
      expect(exitCode).toBe(0);
      await waitForTarget(tree.folder, "After the second launch");
      await waitForInspectionComplete();
      await waitForRowWith(String(holder.pid));
      await screenshot("second launch handed over");
    },
  );
});

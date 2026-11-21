import { expect } from "@wdio/globals";
import {
  createFixtureTree,
  startHolder,
  type Holder,
} from "../support/fixtures.ts";
import { operatingSystem } from "../support/platform.ts";
import { itOn } from "../support/results.ts";
import {
  browser,
  click,
  clickButton,
  inspect,
  resetWorkspace,
  screenshot,
  waitForGone,
  waitForNoRowWith,
  waitForRowWith,
  waitForVisible,
  xpath,
} from "../support/ui.ts";

const dialogTitle = () =>
  browser.execute(
    () => document.querySelector("dialog[open] h2")?.textContent?.trim() ?? "",
  );
const dialogText = () =>
  browser.execute(
    () =>
      document
        .querySelector("dialog[open]")
        ?.textContent?.replace(/\s+/g, " ") ?? "",
  );
/** The confirming action of a termination dialog, e.g. "Close node" or "Force quit node". */
const confirmButton = `${xpath.dialog}//*[contains(@class, "modal-actions")]//button[not(normalize-space(.)="Cancel")][last()]`;

async function expectDialogTitle(pattern: RegExp): Promise<void> {
  await waitForVisible(xpath.dialog);
  await browser.waitUntil(async () => pattern.test(await dialogTitle()), {
    timeout: 15_000,
    timeoutMsg: `Dialog title is "${await dialogTitle()}", expected ${pattern}`,
  });
}

/** The results dialog after a termination: the PID is reported as exited, then Done. */
async function expectTerminationResult(pid: number): Promise<void> {
  await expectDialogTitle(/^Termination results$/);
  await browser.waitUntil(
    async () =>
      /Process exited/.test(await dialogText()) &&
      (await dialogText()).includes(`PID ${pid}`),
    {
      timeout: 20_000,
      timeoutMsg: `Termination results: ${await dialogText()}`,
    },
  );
  await screenshot(`termination result for ${pid}`);
  await clickButton("Done", xpath.dialog);
  await waitForGone(xpath.dialog);
}

/** Select exactly the rows of `pids` by clicking the first and adding the rest with the modifier. */
async function selectOnly(pid: number): Promise<void> {
  await click(xpath.rowWith(String(pid)));
  await waitForVisible(xpath.containsText("1 process selected"));
}

describe("Ending processes", () => {
  const tree = createFixtureTree("terminate", [
    "one.txt",
    "two.txt",
    "three.txt",
    "four.txt",
  ]);
  // On Windows a normal close posts WM_CLOSE to the process windows, so the
  // holder that must close normally there has a real window.
  const closableOptions = { windowed: operatingSystem === "windows" };
  let closable: Holder;
  let forceable: Holder;
  let spare: Holder;

  before(async () => {
    closable = await startHolder([tree.file("one.txt")], closableOptions);
    forceable = await startHolder([tree.file("two.txt")]);
    spare = await startHolder([tree.file("three.txt")]);
    await resetWorkspace();
    await inspect(tree.folder);
    for (const holder of [closable, forceable, spare])
      await waitForRowWith(String(holder.pid));
  });

  it("Terminate… previews the impact and Cancel leaves the process running", async () => {
    await selectOnly(spare.pid);
    await clickButton("Terminate…");
    await expectDialogTitle(/^Close .+\?$/);
    expect(await dialogText()).toContain(`PID ${spare.pid}`);
    await screenshot("close confirmation");
    await clickButton("Cancel", xpath.dialog);
    await waitForGone(xpath.dialog);
    await browser.pause(1_000);
    expect(spare.isRunning()).toBe(true);
    await waitForRowWith(String(spare.pid));
  });

  it("the safety summary in the confirmation expands", async () => {
    await clickButton("Terminate…");
    await expectDialogTitle(/^Close .+\?$/);
    await click(
      `${xpath.dialog}//summary[normalize-space(.)="How OFLH keeps this safe"]`,
    );
    await waitForVisible(
      `${xpath.dialog}//*[contains(normalize-space(.), "Every selected process is listed")]`,
    );
    await screenshot("safety summary");
    await click(`${xpath.dialog}//button[@aria-label="Close dialog"]`);
    await waitForGone(xpath.dialog);
  });

  it("selecting several processes asks to close all of them", async () => {
    await clickButton("Select all");
    await clickButton("Terminate…");
    await expectDialogTitle(/^Close 3 processes\?$/);
    const text = await dialogText();
    for (const holder of [closable, forceable, spare])
      expect(text).toContain(`PID ${holder.pid}`);
    await screenshot("close three processes");
    await clickButton("Cancel", xpath.dialog);
    await waitForGone(xpath.dialog);
    await clickButton("Deselect all");
  });

  it("Terminate process… in the details panel opens the same confirmation", async () => {
    await selectOnly(spare.pid);
    await waitForVisible(xpath.detailsPanel);
    await clickButton("Terminate process…", xpath.detailsPanel);
    await expectDialogTitle(/^Close .+\?$/);
    expect(await dialogText()).toContain(`PID ${spare.pid}`);
    await clickButton("Cancel", xpath.dialog);
    await waitForGone(xpath.dialog);
  });

  it("confirming Close ends the process and removes its row", async () => {
    if (operatingSystem === "windows") {
      await browser.waitUntil(
        () => closable.mainWindowTitle() === "OFLH E2E windowed holder",
        {
          timeout: 15_000,
          timeoutMsg: `The windowed holder still has title ${JSON.stringify(closable.mainWindowTitle())}`,
        },
      );
    }
    await selectOnly(closable.pid);
    await clickButton("Terminate…");
    await expectDialogTitle(/^Close .+\?$/);
    if (operatingSystem === "windows") {
      expect(await dialogText()).toContain("OFLH E2E windowed holder");
    }
    await click(confirmButton);
    expect(await closable.waitForExit(20_000)).toBe(true);
    await expectTerminationResult(closable.pid);
    await waitForNoRowWith(String(closable.pid));
    await screenshot("closed process removed");
  });

  it("Force terminate… stops the process immediately", async () => {
    await selectOnly(forceable.pid);
    await clickButton("Force terminate…");
    await expectDialogTitle(/^Force quit .+\?$/);
    await screenshot("force quit confirmation");
    await click(confirmButton);
    expect(await forceable.waitForExit(20_000)).toBe(true);
    await expectTerminationResult(forceable.pid);
    await waitForNoRowWith(String(forceable.pid));
  });

  itOn(
    ["windows"],
    "Linux and macOS end a windowless process with SIGTERM, so a normal close cannot be refused there",
    "a process without windows cannot close normally and OFLH offers Force terminate",
    async () => {
      const windowless = await startHolder([tree.file("four.txt")]);
      await click(xpath.titled("Refresh ("));
      await waitForRowWith(String(windowless.pid));
      await selectOnly(windowless.pid);
      await clickButton("Terminate…");
      await expectDialogTitle(/^Close .+\?$/);
      await click(confirmButton);
      await waitForVisible(
        xpath.containsText("Some processes could not be stopped normally"),
        30_000,
      );
      await screenshot("normal close refused");
      expect(windowless.isRunning()).toBe(true);
      await click(
        `//button[starts-with(normalize-space(.), "Force terminate")]`,
      );
      await expectDialogTitle(/^Force quit .+\?$/);
      await click(confirmButton);
      expect(await windowless.waitForExit(20_000)).toBe(true);
      await expectTerminationResult(windowless.pid);
    },
  );
});

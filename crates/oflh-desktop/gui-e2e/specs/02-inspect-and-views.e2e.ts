import { expect } from "@wdio/globals";
import {
  createFixtureTree,
  startHolder,
  type Holder,
} from "../support/fixtures.ts";
import {
  $,
  activeView,
  browser,
  click,
  clickButton,
  inspect,
  literal,
  press,
  resetWorkspace,
  rowTexts,
  screenshot,
  switchView,
  targetInput,
  waitForGone,
  waitForInspectionComplete,
  waitForNoRowWith,
  waitForRowWith,
  waitForVisible,
  xpath,
} from "../support/ui.ts";

describe("Inspecting a folder and switching views", () => {
  const tree = createFixtureTree("views", ["alpha.txt", "beta.txt"]);
  let alphaHolder: Holder;
  let betaHolder: Holder;

  before(async () => {
    alphaHolder = await startHolder([tree.file("alpha.txt")]);
    betaHolder = await startHolder([tree.file("beta.txt"), tree.outside]);
    await resetWorkspace();
  });

  it("typing a folder path and pressing Inspect lists every process using it", async () => {
    await inspect(tree.folder);
    await waitForRowWith(String(alphaHolder.pid));
    await waitForRowWith(String(betaHolder.pid));
    await screenshot("processes using the folder");
  });

  it("File usages lists each opened file inside the target and nothing outside it", async () => {
    await switchView("File usages");
    await browser.waitUntil(
      async () => {
        const rows = (await rowTexts()).join("\n");
        return (
          rows.includes(tree.file("alpha.txt")) &&
          rows.includes(tree.file("beta.txt"))
        );
      },
      {
        timeout: 20_000,
        timeoutMsg: "File usages did not list alpha.txt and beta.txt",
      },
    );
    expect((await rowTexts()).join("\n")).not.toContain(tree.outside);
    await screenshot("file usages");
  });

  it("Ports lists local bindings and hides the file target bar", async () => {
    await switchView("Ports");
    await waitForVisible(xpath.grid, 60_000);
    await waitForGone('input[aria-label="Target file or folder path"]');
    await screenshot("ports");
    await switchView("Processes");
    await waitForVisible('input[aria-label="Target file or folder path"]');
    await waitForRowWith(String(alphaHolder.pid));
  });

  it("Recent targets lists the inspected folder and inspects it again on click", async () => {
    await switchView("Recent targets");
    const entry = `//*[contains(normalize-space(.), ${literal(tree.folder)})][self::button or @role="button" or self::a]`;
    await waitForVisible(entry);
    await screenshot("recent targets");
    await click(entry);
    await waitForInspectionComplete();
    await browser.waitUntil(async () => (await activeView()) === "Processes", {
      timeoutMsg: "Opening a recent target did not return to Processes",
    });
    await waitForRowWith(String(betaHolder.pid));
  });

  it("view shortcuts switch between Processes, File usages, Ports and Recent targets", async () => {
    const expectView = async (name: string) =>
      browser.waitUntil(async () => (await activeView()) === name, {
        timeout: 10_000,
        timeoutMsg: `The shortcut did not open ${name}; active view is ${await activeView()}`,
      });
    await $(xpath.grid).click();
    await press("command", "2");
    await expectView("File usages");
    await press("command", "4");
    await expectView("Recent targets");
    await press("command", "3");
    await expectView("Ports");
    await press("command", "1");
    await expectView("Processes");
  });

  it("Refresh picks up a process that started using the folder after the scan", async () => {
    const lateHolder = await startHolder([tree.file("alpha.txt")]);
    await click(xpath.titled("Refresh ("));
    await waitForInspectionComplete();
    await waitForRowWith(String(lateHolder.pid));
    await screenshot("refresh found the new process");
    await lateHolder.stop();
    await $(xpath.grid).click();
    await press("F5");
    await waitForInspectionComplete();
    await waitForNoRowWith(String(lateHolder.pid));
  });

  it("Close inspection removes the target and results but keeps recent targets", async () => {
    await clickButton("Close inspection");
    await waitForGone(xpath.grid);
    await waitForVisible(xpath.text("Drag a file or folder here"));
    expect(await (await targetInput()).getValue()).toBe("");
    await screenshot("closed inspection");
    await switchView("Recent targets");
    await waitForVisible(
      `//*[contains(normalize-space(.), ${literal(tree.folder)})]`,
    );
  });

  it("filtering recent targets hides entries that do not match", async () => {
    const filter = 'input[aria-label="Search recent targets"]';
    await $(filter).setValue("no-such-recent-target");
    await waitForVisible(xpath.text("No matching recent targets"));
    await screenshot("no matching recent targets");
    await $(filter).setValue("oflh-e2e-views");
    await waitForVisible(
      `//button[contains(@class, "recent-target")][@title=${literal(tree.folder)}]`,
    );
  });

  it("removing a recent target deletes only that entry", async () => {
    await click(
      `//button[contains(@class, "recent-remove")][@aria-label=${literal(`Remove ${tree.folder}`)}]`,
    );
    await waitForGone(
      `//button[contains(@class, "recent-target")][@title=${literal(tree.folder)}]`,
    );
  });
});

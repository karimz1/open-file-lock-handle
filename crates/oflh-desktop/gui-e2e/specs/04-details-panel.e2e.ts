import fs from "node:fs";
import path from "node:path";
import { expect } from "@wdio/globals";
import {
  closeFileManagerWindows,
  expectClipboard,
  expectFileManagerToOpen,
  expectFileManagerToReveal,
} from "../support/desktop.ts";
import { sleep } from "../support/app.ts";
import {
  createFixtureTree,
  startHolder,
  type Holder,
} from "../support/fixtures.ts";
import { operatingSystem } from "../support/platform.ts";
import { itKnownFailing } from "../support/results.ts";
import {
  $,
  $$,
  activeView,
  browser,
  click,
  chooseOption,
  clickButton,
  inspect,
  literal,
  resetWorkspace,
  rowTexts,
  targetInput,
  waitForInspectionComplete,
  screenshot,
  waitForGone,
  waitForRowWith,
  waitForVisible,
  xpath,
} from "../support/ui.ts";

const waitPanel = '//section[@aria-label="Selected target wait"]';
const waitPanelStatus = () =>
  browser.execute(
    () =>
      document
        .querySelector(
          'section[aria-label="Selected target wait"] [role=status]',
        )
        ?.textContent?.trim() ?? "",
  );

async function expectWaitStatus(text: string): Promise<void> {
  await browser.waitUntil(async () => (await waitPanelStatus()) === text, {
    timeout: 30_000,
    interval: 250,
    timeoutMsg: `Wait status is "${await waitPanelStatus()}", expected "${text}"`,
  });
}

/** The path shown in a details panel section, e.g. the Executable or Working directory. */
const sectionPath = (heading: string) =>
  browser.execute((title) => {
    const section = [
      ...document.querySelectorAll(
        'aside[aria-label="Process details"] section',
      ),
    ].find((candidate) =>
      candidate.querySelector("h4")?.textContent?.trim().startsWith(title),
    );
    const path = section?.querySelector(".path, code, .mono, [title]");
    return (path?.getAttribute("title") || path?.textContent || "").trim();
  }, heading);

const detailsShowPid = (pid: number) =>
  browser.execute(
    (text) =>
      document
        .querySelector('aside[aria-label="Process details"]')
        ?.textContent?.includes(text) ?? false,
    `PID ${pid}`,
  );

/** Click the process row until the details panel shows that PID; rows may shift while results refresh. */
async function openDetails(pid: number): Promise<void> {
  await browser.waitUntil(
    async () => {
      if (await detailsShowPid(pid)) return true;
      await click(xpath.rowWith(String(pid)));
      await sleep(400);
      return detailsShowPid(pid);
    },
    {
      timeout: 20_000,
      interval: 250,
      timeoutMsg: `Process details for PID ${pid} did not open`,
    },
  );
}

describe("Process details panel", () => {
  const tree = createFixtureTree("details", [
    "alpha.txt",
    "beta.txt",
    "gamma.txt",
    "delta.txt",
  ]);
  let holder: Holder;
  let other: Holder;
  /** Runs inside the target folder, so OFLH lists that folder as its working directory. */
  let insideHolder: Holder;
  /** Runs in the folder above the target, the folder "Inspect owner folder" should move to. */
  let ownerHolder: Holder;

  /** A button of the Matching files entry for `file`. */
  const matchingFileButton = (file: string, name: string) =>
    `${xpath.panelSection("Matching files")}//*[normalize-space(text())=${literal(file)}]/following::button[normalize-space(.)=${literal(name)}][1]`;

  before(async () => {
    holder = await startHolder([tree.file("alpha.txt")]);
    other = await startHolder([tree.file("beta.txt")]);
    insideHolder = await startHolder([tree.file("gamma.txt")], {
      cwd: tree.folder,
    });
    ownerHolder = await startHolder([tree.file("delta.txt")], {
      cwd: path.dirname(tree.folder),
    });
    await resetWorkspace();
    await inspect(tree.folder);
    await waitForRowWith(String(holder.pid));
  });

  afterEach(() => closeFileManagerWindows());

  it("clicking a row opens Process details with the process PID", async () => {
    await openDetails(holder.pid);
    await waitForVisible(
      `${xpath.detailsPanel}//*[contains(normalize-space(.), "PID ${holder.pid}")]`,
    );
    await screenshot("process details");
  });

  it("Copy path in Matching files copies the full file path", async () => {
    await click(matchingFileButton(tree.file("alpha.txt"), "Copy path"));
    await expectClipboard(tree.file("alpha.txt"));
  });

  it("Reveal in Matching files shows the file in the file manager", async () => {
    await click(matchingFileButton(tree.file("alpha.txt"), "Reveal"));
    await expectFileManagerToReveal(tree.file("alpha.txt"));
  });

  it("Reveal for the executable shows the program file in the file manager", async () => {
    const executable = await sectionPath("Executable");
    expect(fs.realpathSync.native(executable)).toBe(
      fs.realpathSync.native(process.execPath),
    );
    await clickButton("Reveal", xpath.panelSection("Executable"));
    await expectFileManagerToReveal(executable);
  });

  // #201: Windows never fills Process.cwd, so this section says Unavailable.
  const copyWorkingDirectory = async () => {
    await openDetails(insideHolder.pid);
    expect(await sectionPath("Working directory")).toBe(tree.folder);
    await clickButton("Copy path", xpath.panelSection("Working directory"));
    await expectClipboard(tree.folder);
  };
  if (operatingSystem === "windows") {
    itKnownFailing(
      201,
      "Copy path for the working directory copies the folder path",
      copyWorkingDirectory,
    );
  } else {
    it(
      "Copy path for the working directory copies the folder path",
      copyWorkingDirectory,
    );
  }

  const revealWorkingDirectory = async () => {
    await openDetails(insideHolder.pid);
    await clickButton("Reveal", xpath.panelSection("Working directory"));
    await expectFileManagerToReveal(tree.folder);
  };
  if (operatingSystem === "windows") {
    itKnownFailing(
      201,
      "Reveal for the working directory shows the folder in the file manager",
      revealWorkingDirectory,
    );
  } else {
    it(
      "Reveal for the working directory shows the folder in the file manager",
      revealWorkingDirectory,
    );
  }

  // #190: a directory handle is still revealed by selecting it in its parent, which
  // can do nothing when that parent is not showable. After #191 this should open the
  // folder itself; turn this back into `it` once that lands.
  itKnownFailing(
    190,
    "Reveal for a folder in Matching files opens the folder itself",
    async () => {
      await click(matchingFileButton(tree.folder, "Reveal"));
      await expectFileManagerToOpen(tree.folder);
    },
  );

  it("the reference summary expands and collapses", async () => {
    await openDetails(holder.pid);
    const summary = `${xpath.panelSection("Matching files")}//summary`;
    const isOpen = () =>
      browser.execute(
        () =>
          document
            .querySelector(
              'aside[aria-label="Process details"] section.matching-files details',
            )
            ?.hasAttribute("open") ?? false,
      );
    await click(summary);
    await browser.waitUntil(isOpen, {
      timeoutMsg: "The reference details did not expand",
    });
    await screenshot("reference details expanded");
    await click(summary);
    await browser.waitUntil(async () => !(await isOpen()), {
      timeoutMsg: "The reference details did not collapse",
    });
  });

  it("selecting an ancestor switches the panel actions to the parent", async () => {
    const ancestry = `${xpath.panelSection("Process ancestry")}//button`;
    const buttons = await $$(ancestry);
    expect(buttons.length).toBeGreaterThan(1);
    await buttons[0].click();
    await waitForVisible(xpath.button("Terminate parent…", xpath.detailsPanel));
    await screenshot("ancestor selected");
    await click(
      `${xpath.panelSection("Process ancestry")}//button[contains(@class, "current-process")]`,
    );
    await waitForVisible(
      xpath.button("Terminate process…", xpath.detailsPanel),
    );
  });

  it("View all matching file usages opens File usages for this process", async () => {
    await clickButton("View all matching file usages", xpath.detailsPanel);
    await browser.waitUntil(
      async () => (await activeView()) === "File usages",
      {
        timeoutMsg: "File usages did not open",
      },
    );
    await browser.waitUntil(
      async () => {
        const rows = (await rowTexts()).join("\n");
        return (
          rows.includes(tree.file("alpha.txt")) &&
          !rows.includes(tree.file("beta.txt"))
        );
      },
      { timeoutMsg: "File usages is not limited to this process" },
    );
    await screenshot("file usages of one process");
    await click(xpath.navigation("Processes"));
    await waitForRowWith(String(other.pid));
  });

  it("Matching handles opens File usages for this process", async () => {
    await openDetails(holder.pid);
    await click(
      `${xpath.detailsPanel}//button[starts-with(normalize-space(.), "Matching handles")]`,
    );
    await browser.waitUntil(
      async () => (await activeView()) === "File usages",
      {
        timeoutMsg: "Matching handles did not open File usages",
      },
    );
    await browser.waitUntil(
      async () =>
        (await rowTexts()).every((row) => !row.includes(tree.file("beta.txt"))),
      { timeoutMsg: "File usages is not limited to this process" },
    );
    await click(xpath.navigation("Processes"));
    await waitForRowWith(String(holder.pid));
  });

  it("the row's details toggle names its action once", async () => {
    const toggle = `${xpath.rowWith(String(holder.pid))}//button[contains(@class, "row-details-toggle")]`;
    const label = await $(toggle).getAttribute("aria-label");
    expect(label).toMatch(/^(Open|Close) details for \S/);
  });

  it("a process with one file usage is counted in the singular", async () => {
    const evidence = await browser.execute(
      (pid) =>
        [...document.querySelectorAll('[role="grid"] [role="row"].data-row')]
          .find((row) => row.textContent?.includes(pid))
          ?.textContent?.match(/\d+ file usages?/)?.[0] ?? "",
      String(holder.pid),
    );
    expect(evidence).toBe("1 file usage");
  });

  it("the close button and the row toggle close and reopen the panel", async () => {
    await openDetails(holder.pid);
    await click('//button[@aria-label="Close process details"]');
    await waitForGone(xpath.detailsPanel);
    const toggle = `${xpath.rowWith(String(holder.pid))}//button[contains(@class, "row-details-toggle")]`;
    await click(toggle);
    await waitForVisible(xpath.detailsPanel);
    await click(toggle);
    await waitForGone(xpath.detailsPanel);
  });

  itKnownFailing(
    194,
    "Rename and show prefills the original name, keeps the extension, and can copy it",
    async () => {
      await openDetails(holder.pid);
      await click(
        matchingFileButton(
          tree.file("alpha.txt"),
          "Wait for file to become free",
        ),
      );
      await waitForVisible(xpath.dialog);
      try {
        await chooseOption("dialog[open] select", "rename");
        const nameField = await $('input[aria-label="New name"]');
        await waitForVisible('input[aria-label="New name"]');
        expect(await nameField.getValue()).toBe("alpha.txt");
        const selected = await browser.execute(() => {
          const field = document.querySelector<HTMLInputElement>(
            'input[aria-label="New name"]',
          )!;
          return field.value.slice(
            field.selectionStart ?? 0,
            field.selectionEnd ?? 0,
          );
        });
        expect(selected).toBe("alpha");
        await clickButton("Copy original name", xpath.dialog);
        await expectClipboard("alpha.txt");
        await nameField.setValue("renamed");
        await waitForVisible(
          `${xpath.dialog}//*[normalize-space(text())="renamed.txt"]`,
        );
        await screenshot("rename keeps extension");
      } finally {
        if (
          await $(xpath.dialog)
            .isDisplayed()
            .catch(() => false)
        ) {
          await click(`${xpath.dialog}//button[@aria-label="Close dialog"]`);
          await waitForGone(xpath.dialog);
        }
      }
    },
  );

  it("Wait for file to become free reports when the holder closes the file", async () => {
    await openDetails(holder.pid);
    await click(
      matchingFileButton(
        tree.file("alpha.txt"),
        "Wait for file to become free",
      ),
    );
    await waitForVisible(xpath.dialog);
    await waitForVisible(
      `${xpath.dialog}//*[normalize-space(text())=${literal(tree.file("alpha.txt"))}]`,
    );
    await screenshot("wait dialog");
    await clickButton("Start waiting", xpath.dialog);
    await waitForGone(xpath.dialog);
    await expectWaitStatus("Waiting until the target is free");
    await holder.release();
    await expectWaitStatus("The target is free.");
    await screenshot("target is free");
    await click(`${waitPanel}//button[@aria-label="Dismiss wait status"]`);
    await waitForGone(waitPanel);
  });

  it("a running wait can be cancelled", async () => {
    await openDetails(other.pid);
    await clickButton("Wait for process exit", xpath.detailsPanel);
    await clickButton("Start waiting", xpath.dialog);
    await expectWaitStatus("Waiting for process exit");
    await clickButton("Cancel", waitPanel);
    await expectWaitStatus("Waiting cancelled.");
    await click(`${waitPanel}//button[@aria-label="Dismiss wait status"]`);
  });

  it("Wait for process exit reports when the process ends", async () => {
    await openDetails(other.pid);
    await clickButton("Wait for process exit", xpath.detailsPanel);
    await clickButton("Start waiting", xpath.dialog);
    await expectWaitStatus("Waiting for process exit");
    await other.stop();
    await expectWaitStatus("The selected process has exited.");
    await screenshot("process exited");
  });

  const inspectOwnerFolder = async () => {
    const ownerFolder = path.dirname(tree.folder);
    await openDetails(ownerHolder.pid);
    expect(await sectionPath("Working directory")).toBe(ownerFolder);
    await clickButton("Inspect owner folder", xpath.detailsPanel);
    await browser.waitUntil(
      async () => (await targetInput().getValue()) === ownerFolder,
      {
        timeoutMsg: "The target did not change to the owner's folder",
      },
    );
    await waitForInspectionComplete();
    await waitForRowWith(String(ownerHolder.pid));
    await screenshot("owner folder inspected");
  };
  if (operatingSystem === "windows") {
    itKnownFailing(
      201,
      "Inspect owner folder starts a new inspection of the owner's working directory",
      inspectOwnerFolder,
    );
  } else {
    it(
      "Inspect owner folder starts a new inspection of the owner's working directory",
      inspectOwnerFolder,
    );
  }
});

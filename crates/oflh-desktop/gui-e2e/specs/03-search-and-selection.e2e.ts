import { expect } from "@wdio/globals";
import { expectClipboard } from "../support/desktop.ts";
import {
  createFixtureTree,
  startHolder,
  type Holder,
} from "../support/fixtures.ts";
import {
  $,
  activeElementDescription,
  browser,
  click,
  clickButton,
  doubleClick,
  inspect,
  press,
  resetWorkspace,
  screenshot,
  searchInput,
  visiblePids,
  waitForAnnouncement,
  waitForRowWith,
  waitForVisible,
  xpath,
} from "../support/ui.ts";

const selectedPids = () =>
  browser.execute(() =>
    [
      ...document.querySelectorAll(
        '[role=grid] [role=row].data-row[aria-selected="true"]',
      ),
    ].map(
      (row) =>
        row.querySelectorAll("[role=gridcell]")[1]?.textContent?.trim() ?? "",
    ),
  );

async function expectPids(expected: number[], message: string): Promise<void> {
  const sorted = [...expected].sort();
  await browser.waitUntil(
    async () =>
      JSON.stringify((await visiblePids()).sort()) === JSON.stringify(sorted),
    {
      timeout: 15_000,
      timeoutMsg: `${message}; grid shows ${(await visiblePids()).join(", ")}`,
    },
  );
}

async function expectSelectedCount(count: number): Promise<void> {
  await browser.waitUntil(async () => (await selectedPids()).length === count, {
    timeout: 10_000,
    timeoutMsg: `Expected ${count} selected rows, found ${(await selectedPids()).length}`,
  });
}

describe("Search and selection", () => {
  const tree = createFixtureTree("search", [
    "alpha.txt",
    "beta.txt",
    "gamma.txt",
  ]);
  let holders: Holder[] = [];
  let pids: number[] = [];

  before(async () => {
    holders = [
      await startHolder([tree.file("alpha.txt")]),
      await startHolder([tree.file("beta.txt")]),
      await startHolder([tree.file("gamma.txt")]),
    ];
    pids = holders.map((holder) => holder.pid);
    await resetWorkspace();
    await inspect(tree.folder);
    for (const pid of pids) await waitForRowWith(String(pid));
  });

  it("searching by PID shows only that process", async () => {
    await (await searchInput()).setValue(String(pids[1]));
    await expectPids([pids[1]], "PID search did not narrow the grid");
    await screenshot("search by pid");
  });

  it("searching by file name shows the processes using a matching path", async () => {
    await (await searchInput()).setValue("gamma");
    await expectPids([pids[2]], "File name search did not narrow the grid");
  });

  it("Clear search shows every process again", async () => {
    await click('//button[@aria-label="Clear search"]');
    expect(await (await searchInput()).getValue()).toBe("");
    await expectPids(pids, "Clearing the search did not restore all rows");
  });

  it("clicking a row selects it and shows the selection bar", async () => {
    await click(xpath.rowWith(String(pids[0])));
    await expectSelectedCount(1);
    await waitForVisible(xpath.containsText("1 process selected"));
    await screenshot("one row selected");
  });

  it("Select all selects every matching process and Deselect all clears it", async () => {
    await clickButton("Select all");
    await expectSelectedCount(3);
    await waitForVisible(xpath.button("Deselect all"));
    await screenshot("all rows selected");
    await click(`//button[contains(@class, "select-all")]`);
    await expectSelectedCount(0);
  });

  it("the select-all shortcut selects every row and Escape clears the selection", async () => {
    await $(xpath.grid).click();
    await press("command", "a");
    await expectSelectedCount(3);
    await press("Escape");
    await expectSelectedCount(0);
  });

  it("Copy puts the selected process on the clipboard", async () => {
    await click(xpath.rowWith(String(pids[1])));
    await expectSelectedCount(1);
    await clickButton("Copy");
    await waitForAnnouncement("Selection copied");
    await expectClipboard(new RegExp(`\\b${pids[1]}\\b`));
  });

  it("the copy shortcut copies the selection", async () => {
    await click(xpath.rowWith(String(pids[2])));
    await press("command", "c");
    await waitForAnnouncement("Selection copied");
    await expectClipboard(new RegExp(`\\b${pids[2]}\\b`));
  });

  // #186, fixed in #187.
  it("typing a search after double-clicking a row keeps focus and every character", async () => {
    await press("Escape");
    await doubleClick(xpath.rowWith(String(pids[0])));
    await waitForVisible(xpath.detailsPanel);
    await (await searchInput()).click();
    for (const character of "alpha") {
      await press(character);
      await browser.pause(150);
    }
    await screenshot("search typed after opening a row");
    const focused = await activeElementDescription();
    const value = await (await searchInput()).getValue();
    if (focused !== "input[Search loaded results]" || value !== "alpha") {
      throw new Error(
        `After typing "alpha", focus is on ${focused} and the search holds "${value}"`,
      );
    }
  });
});

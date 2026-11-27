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
  inspect,
  press,
  resetWorkspace,
  rowTexts,
  screenshot,
  searchInput,
  switchView,
  visiblePids,
  waitForGone,
  waitForInspectionComplete,
  waitForRowWith,
  waitForVisible,
  xpath,
} from "../support/ui.ts";

const manyFiles = Array.from(
  { length: 120 },
  (_, index) => `file-${String(index).padStart(3, "0")}.txt`,
);
const focusedRowIndex = () =>
  browser.execute(() => {
    const grid = document.querySelector("[role=grid]");
    return grid?.getAttribute("aria-activedescendant") ?? "";
  });
const columnHeaders = () =>
  browser.execute(() =>
    [...document.querySelectorAll("[role=grid] [role=columnheader]")].map(
      (cell) => cell.textContent?.trim() ?? "",
    ),
  );

/**
 * Wait until the grid shows the current query's own rows. After a view
 * switch the grid keeps the previous view's rows on screen (read-only, with
 * aria-busy) until the new page arrives, and then scrolls back to the top.
 */
const waitForGridRows = (rows: number) =>
  browser.waitUntil(
    () =>
      browser.execute((count) => {
        const grid = document.querySelector("[role=grid]");
        return (
          grid?.getAttribute("aria-busy") === "false" &&
          grid.getAttribute("aria-rowcount") === String(count + 1)
        );
      }, rows),
    { timeoutMsg: `The grid did not show its ${rows} rows` },
  );

/** Put keyboard focus on the grid itself, as Tab does, without clicking a row. */
const focusGrid = () =>
  browser.execute(() =>
    document.querySelector<HTMLElement>("[role=grid]")!.focus(),
  );

describe("Grid, keyboard and layout", () => {
  const tree = createFixtureTree("grid", [
    ...manyFiles,
    "small.txt",
    "other.txt",
  ]);
  let bulk: Holder;
  let small: Holder;
  let other: Holder;

  before(async () => {
    bulk = await startHolder(manyFiles.map((name) => tree.file(name)));
    small = await startHolder([tree.file("small.txt")]);
    other = await startHolder([tree.file("other.txt")]);
    await resetWorkspace();
    await inspect(tree.folder);
    for (const holder of [bulk, small, other])
      await waitForRowWith(String(holder.pid));
  });

  it("coverage notices expand and collapse", async () => {
    const summary = `//summary[contains(normalize-space(.), "coverage notices")]`;
    const isOpen = () =>
      browser.execute(() => {
        const summaries = [...document.querySelectorAll("summary")];
        const found = summaries.find((element) =>
          element.textContent?.includes("coverage notices"),
        );
        return found?.parentElement?.hasAttribute("open") ?? false;
      });
    await click(summary);
    await browser.waitUntil(isOpen, {
      timeoutMsg: "Coverage notices did not expand",
    });
    await screenshot("coverage notices");
    await click(summary);
    await browser.waitUntil(async () => !(await isOpen()), {
      timeoutMsg: "Coverage notices did not collapse",
    });
  });

  it("sorting by PID orders the rows both ways", async () => {
    await click('//button[@aria-label="Sort by PID"]');
    const ascending = [bulk.pid, small.pid, other.pid].sort((a, b) => a - b);
    await browser.waitUntil(
      async () =>
        JSON.stringify(await visiblePids()) === JSON.stringify(ascending),
      { timeoutMsg: `PID sort is not ascending: ${await visiblePids()}` },
    );
    await click('//button[@aria-label="Sort by PID"]');
    await browser.waitUntil(
      async () =>
        JSON.stringify(await visiblePids()) ===
        JSON.stringify([...ascending].reverse()),
      { timeoutMsg: `PID sort is not descending: ${await visiblePids()}` },
    );
  });

  it("the Columns menu hides and shows a column", async () => {
    await click('//details[contains(@class, "column-picker")]/summary');
    const memory =
      '//*[@role="group"][@aria-label="Visible columns"]//label[normalize-space(.)="Memory"]';
    await click(memory);
    await browser.waitUntil(
      async () => !(await columnHeaders()).includes("Memory"),
      {
        timeoutMsg: "Memory is still shown",
      },
    );
    await screenshot("memory column hidden");
    await click(memory);
    await browser.waitUntil(
      async () => (await columnHeaders()).includes("Memory"),
      {
        timeoutMsg: "Memory did not come back",
      },
    );
    await press("Escape");
  });

  it("Lock evidence only filters the rows and unchecking restores them", async () => {
    const before = (await visiblePids()).length;
    await click('//label[normalize-space(.)="Lock evidence only"]');
    await browser.waitUntil(async () => (await visiblePids()).length < before, {
      timeoutMsg: "Lock evidence only did not filter plain open files",
    });
    await screenshot("lock evidence only");
    await click('//label[normalize-space(.)="Lock evidence only"]');
    await browser.waitUntil(
      async () => (await visiblePids()).length === before,
      {
        timeoutMsg: "Unchecking Lock evidence only did not restore the rows",
      },
    );
  });

  it("Column filters open and close", async () => {
    await click(xpath.titled("Open column filters"));
    await waitForVisible('//*[contains(@class, "column-filters")]');
    await screenshot("column filters");
    await click(xpath.titled("Close column filters")).catch(() =>
      click(xpath.titled("Open column filters")),
    );
    await waitForGone('//*[contains(@class, "column-filters")]');
  });

  it("Maximize grid hides the sidebar and Escape restores the layout", async () => {
    await click('//button[@aria-label="Maximize grid"]');
    await waitForVisible(".app-body.grid-maximized");
    await screenshot("maximized grid");
    await $(xpath.grid).click();
    await press("Escape");
    await waitForGone(".app-body.grid-maximized");
  });

  it("the search shortcut focuses the search box", async () => {
    await $(xpath.grid).click();
    await press("command", "f");
    await browser.waitUntil(
      async () =>
        (await activeElementDescription()) === "input[Search loaded results]",
      { timeoutMsg: `Focus is on ${await activeElementDescription()}` },
    );
    await (await searchInput()).setValue("");
    await press("Escape");
  });

  it("Shift+F10 opens the row's context actions and Copy filename copies it", async () => {
    await focusGrid();
    await press("Home");
    await browser.keys(["Shift", "F10"]);
    await waitForVisible(
      `${xpath.dialog}//*[contains(normalize-space(.), "Actions for PID")]`,
    );
    await screenshot("row context actions");
    await clickButton("Copy filename", xpath.dialog);
    await waitForGone(xpath.dialog);
    await expectClipboard(/^[\w.-]+\.txt$/);
  });

  it("arrow keys move through the rows and Enter opens details", async () => {
    await press("Escape");
    await focusGrid();
    await press("Home");
    const first = await focusedRowIndex();
    await press("ArrowDown");
    await browser.waitUntil(async () => (await focusedRowIndex()) !== first, {
      timeoutMsg: "ArrowDown did not move the row cursor",
    });
    await press("Enter");
    await waitForVisible(xpath.detailsPanel);
    await screenshot("details opened with Enter");
    await click('//button[@aria-label="Close process details"]');
  });

  // #186, fixed in #187.
  it("after Enter opened details once, arrow keys keep focus in the grid", async () => {
    await focusGrid();
    await press("Home");
    await press("ArrowDown");
    await browser.pause(300);
    const focused = await activeElementDescription();
    if (!focused.startsWith("div[Processes using this target]")) {
      throw new Error(
        `Keyboard focus moved to ${focused} after opening a row with the keyboard`,
      );
    }
  });

  it("scrolling File usages reaches the last of many virtualized rows", async () => {
    await switchView("File usages");
    await waitForVisible(xpath.grid);
    const total = manyFiles.length + 2;
    await waitForGridRows(total);
    expect((await rowTexts()).length).toBeLessThan(total);
    // Record every scroll position the grid goes through, so a failure says
    // whether the scroll never landed or something moved the grid back.
    await browser.execute(() => {
      const grid = document.querySelector<HTMLElement>("[role=grid]")!;
      const trace: string[] = [];
      (window as unknown as { oflhScrollTrace: string[] }).oflhScrollTrace =
        trace;
      grid.addEventListener("scroll", () =>
        trace.push(`${Math.round(performance.now())}ms top=${grid.scrollTop}`),
      );
      trace.push(
        `scroll from top=${grid.scrollTop} to ${grid.scrollHeight} (client ${grid.clientHeight}, rows ${grid.getAttribute("aria-rowcount")}, busy ${grid.getAttribute("aria-busy")})`,
      );
      grid.scrollTop = grid.scrollHeight;
    });
    const lastRowsShown = async () =>
      (await rowTexts()).some(
        (row) =>
          row.includes("file-119.txt") ||
          row.includes("small.txt") ||
          row.includes("other.txt"),
      );
    await browser
      .waitUntil(lastRowsShown, { timeout: 20_000 })
      .catch(async () => {
        const trace = await browser.execute(() =>
          (
            window as unknown as { oflhScrollTrace: string[] }
          ).oflhScrollTrace.join("; "),
        );
        throw new Error(
          `The last rows did not render after scrolling to the end. Scroll trace: ${trace}`,
        );
      });
    await screenshot("scrolled to the end");
    await $(xpath.grid).click();
    await press("Home");
    await browser.waitUntil(
      async () =>
        (await rowTexts()).some((row) => row.includes("file-000.txt")),
      {
        timeoutMsg: "Home did not return to the first row",
      },
    );
    await press("End");
    await browser.waitUntil(
      async () =>
        !(await rowTexts()).some((row) => row.includes("file-000.txt")),
      {
        timeoutMsg: "End did not move to the last row",
      },
    );
    await switchView("Processes");
  });

  it("automatic refresh finds a new process without a manual refresh", async () => {
    await switchView("Processes");
    await click('//button[@aria-label="Automatic refresh interval"]');
    await click('//*[@role="option"][normalize-space(.)="5s"]');
    const late = await startHolder([tree.file("other.txt")]);
    await waitForRowWith(String(late.pid), 30_000);
    await screenshot("automatic refresh found a process");
    await click('//button[@aria-label="Automatic refresh interval"]');
    await click('//*[@role="option"][normalize-space(.)="Off"]');
    await waitForInspectionComplete();
  });
});

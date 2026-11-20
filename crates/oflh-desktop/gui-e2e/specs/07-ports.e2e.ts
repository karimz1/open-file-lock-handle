import { expect } from "@wdio/globals";
import { expectClipboard } from "../support/desktop.ts";
import { startHolder, type Holder } from "../support/fixtures.ts";
import {
  browser,
  click,
  clickButton,
  press,
  resetWorkspace,
  rowTexts,
  screenshot,
  searchInput,
  switchView,
  waitForGone,
  waitForRowWith,
  waitForVisible,
  xpath,
} from "../support/ui.ts";

describe("Ports view", () => {
  let listener: Holder;

  before(async () => {
    listener = await startHolder([], { listen: true });
    await resetWorkspace();
  });

  it("Ports lists the fixture's TCP listener with its PID", async () => {
    await switchView("Ports");
    await waitForRowWith(String(listener.pid), 60_000);
    await screenshot("ports with the fixture listener");
  });

  it("searching for the port number narrows the list to that binding", async () => {
    await (await searchInput()).setValue(String(listener.port));
    await browser.waitUntil(
      async () => {
        const rows = await rowTexts();
        return (
          rows.length >= 1 &&
          rows.every((row) => row.includes(String(listener.port)))
        );
      },
      { timeoutMsg: `Port search shows ${JSON.stringify(await rowTexts())}` },
    );
    await waitForRowWith(String(listener.pid));
    await screenshot("port search");
  });

  it("clicking the binding opens details for the owning process", async () => {
    await click(xpath.rowWith(String(listener.pid)));
    await waitForVisible(xpath.detailsPanel);
    await waitForVisible(
      `${xpath.detailsPanel}//*[contains(normalize-space(.), "PID ${listener.pid}")]`,
    );
    await screenshot("port details");
  });

  it("the binding's context actions copy the port", async () => {
    await browser.execute(() =>
      document.querySelector<HTMLElement>("[role=grid]")!.focus(),
    );
    await press("Home");
    await browser.keys(["Shift", "F10"]);
    await waitForVisible(
      `${xpath.dialog}//*[contains(normalize-space(.), "Actions for PID ${listener.pid}")]`,
    );
    await clickButton("Copy port", xpath.dialog);
    await waitForGone(xpath.dialog);
    await expectClipboard(new RegExp(`\\b${listener.port}\\b`));
  });

  it("Wait for port to become free reports when the listener closes", async () => {
    await browser.execute(() =>
      document.querySelector<HTMLElement>("[role=grid]")!.focus(),
    );
    await press("Home");
    await browser.keys(["Shift", "F10"]);
    await clickButton("Wait for port to become free", xpath.dialog);
    await clickButton("Start waiting", xpath.dialog);
    await waitForGone(xpath.dialog);
    const status = () =>
      browser.execute(
        () =>
          document
            .querySelector(
              'section[aria-label="Selected target wait"] [role=status]',
            )
            ?.textContent?.trim() ?? "",
      );
    await browser.waitUntil(
      async () => (await status()) === "Waiting until the target is free",
      {
        timeoutMsg: `Wait status: ${await status()}`,
      },
    );
    await listener.release();
    await browser.waitUntil(
      async () => (await status()) === "The target is free.",
      {
        timeout: 30_000,
        timeoutMsg: `Wait status: ${await status()}`,
      },
    );
    await screenshot("port is free");
    expect(listener.isRunning()).toBe(true);
  });
});

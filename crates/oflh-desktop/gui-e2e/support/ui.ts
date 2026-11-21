import fs from "node:fs";
import path from "node:path";
import { $, $$, browser } from "@wdio/globals";
import { screenshotsDir } from "./paths.ts";
import { commandKey } from "./platform.ts";

/**
 * Small, readable locators and actions for the OFLH window. The embedded
 * WebDriver server supports CSS and XPath; element references cannot be
 * returned from scripts, so everything is located by role, label or text.
 */

/** XPath string literal that survives both quote characters. */
export function literal(text: string): string {
  if (!text.includes('"')) return `"${text}"`;
  if (!text.includes("'")) return `'${text}'`;
  return `concat(${text
    .split('"')
    .map((part) => `"${part}"`)
    .join(`, '"', `)})`;
}

const named = (name: string) =>
  `(normalize-space(@aria-label)=${literal(name)} or normalize-space(.)=${literal(name)} or normalize-space(@title)=${literal(name)})`;

export const xpath = {
  button: (name: string, within = "") => `${within}//button[${named(name)}]`,
  /** Sidebar entry whose tooltip starts with the view name, e.g. "Processes (Ctrl+1)". */
  navigation: (view: string) =>
    `//nav//button[starts-with(normalize-space(@title), ${literal(`${view} (`)})]`,
  /** A button whose tooltip starts with `prefix`, e.g. "Refresh (" for "Refresh (Ctrl+R or F5)". */
  titled: (prefix: string) =>
    `//button[starts-with(normalize-space(@title), ${literal(prefix)})]`,
  dialog: "//dialog[@open]",
  detailsPanel: '//aside[@aria-label="Process details"]',
  grid: '//*[@role="grid"]',
  dataRows:
    '//*[@role="grid"]//*[@role="row"][contains(concat(" ", @class, " "), " data-row ")]',
  /** The grid row that shows `text` in one of its cells, e.g. a PID. */
  rowWith: (text: string) =>
    `//*[@role="grid"]//*[@role="row"][contains(concat(" ", @class, " "), " data-row ")][.//*[normalize-space(text())=${literal(text)}]]`,
  /** A section of the details panel by its heading, e.g. "Working directory". */
  panelSection: (heading: string) =>
    `//aside[@aria-label="Process details"]//section[.//h4[starts-with(normalize-space(.), ${literal(heading)})]]`,
  text: (text: string) => `//*[normalize-space(text())=${literal(text)}]`,
  containsText: (text: string) =>
    `//*[contains(normalize-space(.), ${literal(text)}) and not(*[contains(normalize-space(.), ${literal(text)})])]`,
};

export const targetInput = () =>
  $('input[aria-label="Target file or folder path"]');
export const searchInput = () => $('input[aria-label="Search loaded results"]');

export async function click(locator: string): Promise<void> {
  const element = await $(locator);
  await element.waitForClickable({
    timeout: 15_000,
    timeoutMsg: `Not clickable: ${locator}`,
  });
  await element.click();
}

/**
 * Double-click like a browser does: two clicks, then a `dblclick` event. The
 * embedded driver performs the clicks but does not synthesize `dblclick`.
 */
export async function doubleClick(locator: string): Promise<void> {
  await click(locator);
  await (await $(locator)).click();
  await browser.execute((path) => {
    const element = document.evaluate(
      path,
      document,
      null,
      XPathResult.FIRST_ORDERED_NODE_TYPE,
      null,
    ).singleNodeValue as Element | null;
    if (!element) throw new Error(`Nothing to double-click at ${path}`);
    const box = element.getBoundingClientRect();
    element.dispatchEvent(
      new MouseEvent("dblclick", {
        bubbles: true,
        cancelable: true,
        composed: true,
        detail: 2,
        clientX: box.left + box.width / 2,
        clientY: box.top + box.height / 2,
      }),
    );
  }, locator);
}

export const clickButton = (name: string, within = "") =>
  click(xpath.button(name, within));

/**
 * Whether any element matching `locator` is displayed right now. Looks the
 * element up again on every call: a reference found earlier can belong to a
 * node React has since replaced (for example Termination results swapped for
 * the Force quit confirmation), and a detached node never becomes visible.
 */
async function anyDisplayed(locator: string): Promise<boolean> {
  for (const element of await $$(locator)) {
    if (await element.isDisplayed().catch(() => false)) return true;
  }
  return false;
}

export async function waitForVisible(
  locator: string,
  timeout = 20_000,
): Promise<void> {
  await browser.waitUntil(() => anyDisplayed(locator), {
    timeout,
    interval: 200,
    timeoutMsg: `Not visible: ${locator}`,
  });
}

export async function waitForGone(
  locator: string,
  timeout = 20_000,
): Promise<void> {
  await browser.waitUntil(async () => !(await anyDisplayed(locator)), {
    timeout,
    interval: 200,
    timeoutMsg: `Still visible: ${locator}`,
  });
}

/** The status line at the bottom left, e.g. "Inspection complete". */
export async function statusText(): Promise<string> {
  return browser.execute(
    () =>
      document
        .querySelector("footer.statusbar .status-current")
        ?.textContent?.trim() ?? "",
  );
}

/** Wait until any live region announces `text` (toasts and the status bar). */
export async function waitForAnnouncement(
  text: string | RegExp,
  timeout = 20_000,
): Promise<void> {
  let seen: string[] = [];
  await browser.waitUntil(
    async () => {
      seen = await browser.execute(() =>
        [
          ...document.querySelectorAll(
            "[role=status], [role=alert], [aria-live]",
          ),
        ].map((element) => element.textContent?.trim() ?? ""),
      );
      return seen.some((value) =>
        typeof text === "string" ? value.includes(text) : text.test(value),
      );
    },
    {
      timeout,
      interval: 200,
      timeoutMsg: `No announcement ${text}; saw ${JSON.stringify(seen)}`,
    },
  );
}

/** Type a path into the target bar and press its Inspect button. */
export async function inspect(target: string): Promise<void> {
  const input = await targetInput();
  await input.waitForDisplayed({ timeout: 20_000 });
  await input.setValue(target);
  await clickButton("Inspect");
  await waitForInspectionComplete();
}

export async function waitForInspectionComplete(
  timeout = 120_000,
): Promise<void> {
  await browser.waitUntil(
    async () =>
      /^(Inspection complete|Results refreshed)/.test(await statusText()),
    {
      timeout,
      interval: 250,
      timeoutMsg: `Inspection did not complete; status is "${await statusText()}"`,
    },
  );
}

/** Text of every rendered data row in the grid. */
export async function rowTexts(): Promise<string[]> {
  return browser.execute(() =>
    [...document.querySelectorAll("[role=grid] [role=row].data-row")].map(
      (row) => row.textContent ?? "",
    ),
  );
}

/** PIDs shown in the PID column of the rendered rows. */
export async function visiblePids(): Promise<number[]> {
  const texts = await browser.execute(() => {
    const header = [
      ...document.querySelectorAll("[role=grid] [role=columnheader]"),
    ];
    const index = header.findIndex((cell) =>
      cell.textContent?.trim().startsWith("PID"),
    );
    return [
      ...document.querySelectorAll("[role=grid] [role=row].data-row"),
    ].map((row) => {
      const cells = row.querySelectorAll("[role=gridcell]");
      return (index >= 0 ? cells[index]?.textContent : "")?.trim() ?? "";
    });
  });
  return texts.map(Number).filter((pid) => Number.isInteger(pid) && pid > 0);
}

export async function waitForRowWith(
  text: string,
  timeout = 30_000,
): Promise<void> {
  await waitForVisible(xpath.rowWith(text), timeout);
}

export async function waitForNoRowWith(
  text: string,
  timeout = 30_000,
): Promise<void> {
  await browser.waitUntil(
    async () => !(await $(xpath.rowWith(text)).isExisting()),
    {
      timeout,
      interval: 250,
      timeoutMsg: `A row with ${text} is still shown`,
    },
  );
}

export async function switchView(
  view: "Processes" | "File usages" | "Ports" | "Recent targets",
) {
  await click(xpath.navigation(view));
  await browser.waitUntil(
    async () =>
      browser.execute(
        (name) =>
          [...document.querySelectorAll("nav button.active")].some((button) =>
            button.getAttribute("title")?.startsWith(`${name} (`),
          ),
        view,
      ),
    { timeout: 10_000, timeoutMsg: `${view} did not become the active view` },
  );
}

export async function activeView(): Promise<string> {
  return browser.execute(
    () =>
      document
        .querySelector("nav button.active")
        ?.getAttribute("title")
        ?.replace(/ \(.*$/, "") ?? "",
  );
}

/** Open Settings through the gear menu, the way a user does. */
export async function openSettings(): Promise<void> {
  await click('//nav//button[@aria-label="Settings"]');
  await click(
    '//*[@role="menuitem"][starts-with(normalize-space(.), "Settings")]',
  );
  await waitForVisible("#language");
}

/**
 * Set a `<select>` the way a user's choice reaches React: the embedded driver
 * clicks options with synthetic events, which do not change a native select.
 */
export async function chooseOption(
  selector: string,
  value: string,
): Promise<void> {
  await $(selector).waitForDisplayed({ timeout: 10_000 });
  // Defer the change event so this command can return before a language
  // switch reloads the page (#188). Dispatching change synchronously made
  // WebView2 reject the script with "Illegal invocation".
  await browser.execute(
    (css, next) => {
      const select = document.querySelector<HTMLSelectElement>(css);
      if (!select) return;
      const setter = Object.getOwnPropertyDescriptor(
        Object.getPrototypeOf(select),
        "value",
      )?.set;
      if (setter) setter.call(select, next);
      else select.value = next;
      setTimeout(() => {
        select.dispatchEvent(new Event("input", { bubbles: true }));
        select.dispatchEvent(new Event("change", { bubbles: true }));
      }, 0);
    },
    selector,
    value,
  );
}

/** Press a shortcut like `command+1` or `Escape`; `command` is Cmd on macOS and Ctrl elsewhere. */
export async function press(...keys: string[]): Promise<void> {
  await browser.keys(keys.map((key) => (key === "command" ? commandKey : key)));
}

/** Clear WebView storage and reload, so every spec starts in English with a known theme. */
export async function resetWorkspace({
  firstLaunch = false,
} = {}): Promise<void> {
  await browser.execute((fresh) => {
    localStorage.clear();
    localStorage.setItem("oflh-language", "en");
    if (!fresh) localStorage.setItem("oflh-theme", "vscode");
  }, firstLaunch);
  await reload();
}

/**
 * Reload the page and wait until the workspace is back.
 * Do not call `location.reload()` inside `execute/sync`: on WebView2 the page
 * is torn down before the command can return, so the script times out (30 s)
 * even though the reload itself succeeded.
 */
export async function reload(): Promise<void> {
  await browser.refresh();
  await browser.waitUntil(
    async () => {
      try {
        return await browser.execute(
          () =>
            document.readyState === "complete" &&
            !!document.querySelector("nav"),
        );
      } catch {
        return false;
      }
    },
    {
      timeout: 30_000,
      interval: 250,
      timeoutMsg: "The workspace did not load after a reload",
    },
  );
}

/** Close an open modal or native dialog so a failed test does not block the next one. */
export async function dismissOpenDialog(): Promise<void> {
  const open = await $(xpath.dialog)
    .isDisplayed()
    .catch(() => false);
  if (!open) return;
  const close = `${xpath.dialog}//button[@aria-label="Close dialog" or normalize-space(.)="Cancel" or normalize-space(.)="Done"]`;
  if (
    await $(close)
      .isExisting()
      .catch(() => false)
  ) {
    await click(close).catch(() => undefined);
  }
  await waitForGone(xpath.dialog).catch(() => undefined);
}

export async function activeElementDescription(): Promise<string> {
  return browser.execute(() => {
    const element = document.activeElement;
    if (!element) return "none";
    return `${element.tagName.toLowerCase()}${element.getAttribute("aria-label") ? `[${element.getAttribute("aria-label")}]` : ""}`;
  });
}

let screenshotIndex = 0;

/** Save a numbered screenshot into the run's artifacts; returns its path. */
export async function screenshot(name: string): Promise<string> {
  fs.mkdirSync(screenshotsDir, { recursive: true });
  screenshotIndex += 1;
  const safe = name
    .replace(/[^a-z0-9]+/gi, "-")
    .replace(/^-|-$/g, "")
    .toLowerCase();
  const file = path.join(
    screenshotsDir,
    `${process.env.OFLH_E2E_SPEC ?? "spec"}-${String(screenshotIndex).padStart(2, "0")}-${safe}.png`,
  );
  await browser.saveScreenshot(file);
  return file;
}

export { $, $$, browser };

import fs from "node:fs";
import path from "node:path";
import { expect } from "@wdio/globals";
import { portableLayout } from "../support/app.ts";
import { itOn } from "../support/results.ts";
import {
  browser,
  clickButton,
  openSettings,
  resetWorkspace,
  screenshot,
  waitForGone,
  waitForVisible,
  xpath,
} from "../support/ui.ts";

const appliedTheme = () =>
  browser.execute(() => document.documentElement.dataset.theme ?? "");

function listing(directory: string): string[] {
  if (!fs.existsSync(directory)) return [];
  const names: string[] = [];
  for (const entry of fs.readdirSync(directory, { recursive: true })) {
    names.push(String(entry));
  }
  names.sort();
  return names;
}

function bundleNames(root: string): string[] {
  if (!fs.existsSync(root)) return [];
  return fs
    .readdirSync(root)
    .filter((name) => name.startsWith("io.github.karimz1.oflh"))
    .sort();
}

itOn(
  ["windows"],
  "portable zips are a Windows package",
  "portable mode stores settings next to the exe and leaves AppData unchanged",
  async () => {
    const {
      root,
      data,
      beforeAppData,
      beforeLocal,
      beforeAppDataBundles,
      beforeLocalBundles,
    } = portableLayout();
    const appData = path.join(
      process.env.APPDATA ?? "",
      "io.github.karimz1.oflh",
    );
    const localData = path.join(
      process.env.LOCALAPPDATA ?? "",
      "io.github.karimz1.oflh",
    );

    await resetWorkspace({ firstLaunch: true });
    await waitForVisible(
      `${xpath.dialog}[.//h2[normalize-space()="Make OFLH yours"]]`,
    );
    await clickButton("Light", xpath.dialog);
    await browser.waitUntil(async () => (await appliedTheme()) === "light", {
      timeoutMsg: "Choosing Light did not preview the light theme",
    });
    await clickButton("Apply theme", xpath.dialog);
    await waitForGone(xpath.dialog);
    expect(
      await browser.execute(() => localStorage.getItem("oflh-theme")),
    ).toBe("light");
    await openSettings();
    await waitForVisible("#language");
    await screenshot("portable settings");

    expect(fs.existsSync(path.join(root, "portable"))).toBe(true);
    expect(fs.existsSync(path.join(data, "oflh.sqlite3"))).toBe(true);
    const webviewUsed =
      fs.existsSync(path.join(data, "webview")) ||
      listing(data).some((name) => name.toLowerCase().includes("ebwebview"));
    expect(webviewUsed).toBe(true);
    expect(listing(appData)).toEqual(beforeAppData);
    expect(listing(localData)).toEqual(beforeLocal);
    expect(bundleNames(process.env.APPDATA ?? "")).toEqual(
      beforeAppDataBundles,
    );
    expect(bundleNames(process.env.LOCALAPPDATA ?? "")).toEqual(
      beforeLocalBundles,
    );
  },
);

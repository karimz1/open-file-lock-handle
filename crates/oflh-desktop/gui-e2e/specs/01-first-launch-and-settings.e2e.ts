import { expect } from "@wdio/globals";
import {
  activeView,
  browser,
  chooseOption,
  click,
  clickButton,
  openSettings,
  reload,
  resetWorkspace,
  screenshot,
  waitForGone,
  waitForVisible,
  xpath,
} from "../support/ui.ts";

const appliedTheme = () =>
  browser.execute(() => document.documentElement.dataset.theme ?? "");
const savedTheme = () =>
  browser.execute(() => localStorage.getItem("oflh-theme"));
const pageLanguage = () => browser.execute(() => document.documentElement.lang);
const rootFontSize = () =>
  browser.execute(() => getComputedStyle(document.documentElement).fontSize);

describe("First launch and Settings", () => {
  it("first launch asks for a theme, previews it and applies it", async () => {
    await resetWorkspace({ firstLaunch: true });
    await waitForVisible(
      `${xpath.dialog}[.//h2[normalize-space()="Make OFLH yours"]]`,
    );
    await screenshot("theme chooser on first launch");
    await clickButton("Light", xpath.dialog);
    await browser.waitUntil(async () => (await appliedTheme()) === "light", {
      timeoutMsg: "Choosing Light did not preview the light theme",
    });
    await clickButton("Apply theme", xpath.dialog);
    await waitForGone(xpath.dialog);
    expect(await savedTheme()).toBe("light");
    await screenshot("light theme applied");
  });

  it("the gear menu opens Settings", async () => {
    await resetWorkspace();
    await openSettings();
    await waitForVisible(xpath.button("Rider Dark"));
    await screenshot("settings");
  });

  it("choosing a theme in Settings applies it in place and persists it", async () => {
    await clickButton("Rider Dark");
    await browser.waitUntil(async () => (await appliedTheme()) === "rider", {
      timeoutMsg: "Rider Dark was not applied",
    });
    await waitForVisible("#language");
    expect(await savedTheme()).toBe("rider");
    await clickButton("Light");
    await browser.waitUntil(async () => (await appliedTheme()) === "light");
    await reload();
    await browser.waitUntil(async () => (await appliedTheme()) === "light", {
      timeoutMsg: "The chosen theme did not survive a reload",
    });
  });

  it("font size and zoom buttons resize the interface", async () => {
    await openSettings();
    await chooseOption("#font-size", "16");
    await browser.waitUntil(async () => (await rootFontSize()) === "16px", {
      timeoutMsg: "Font size 16 px was not applied",
    });
    await click('//button[@aria-label="Zoom in"]');
    await browser.waitUntil(async () => (await rootFontSize()) === "18px", {
      timeoutMsg: "Zoom in did not grow the font by one 2 px step",
    });
    await click('//button[@aria-label="Zoom out"]');
    await click('//button[@aria-label="Zoom out"]');
    await browser.waitUntil(async () => (await rootFontSize()) === "14px", {
      timeoutMsg: "Zoom out did not shrink the font",
    });
  });

  it("switching the language translates the interface and persists", async () => {
    await openSettings();
    await chooseOption("#language", "de");
    await browser.waitUntil(async () => (await pageLanguage()) === "de", {
      timeoutMsg: "The interface did not switch to German",
    });
    await waitForVisible(xpath.navigation("Prozesse"));
    await screenshot("german interface");
    expect(
      await browser.execute(() => localStorage.getItem("oflh-language")),
    ).toBe("de");
  });

  // #188, fixed in #189.
  it("switching the language keeps Settings open", async () => {
    await resetWorkspace();
    await openSettings();
    await chooseOption("#language", "de");
    await browser.waitUntil(async () => (await pageLanguage()) === "de");
    await screenshot("view after switching to German");
    await waitForVisible("#language", 5_000);
  });

  it("switching back to English restores English labels", async () => {
    await browser.execute(() => localStorage.setItem("oflh-language", "de"));
    await reload();
    await openSettingsGerman();
    await chooseOption("#language", "en");
    await browser.waitUntil(async () => (await pageLanguage()) === "en", {
      timeoutMsg: "The interface did not switch back to English",
    });
    await waitForVisible(xpath.navigation("Processes"));
  });

  it("About OFLH opens from the gear menu and closes", async () => {
    await click('//nav//button[@aria-label="Settings"]');
    await click(
      '//*[@role="menuitem"][starts-with(normalize-space(.), "About OFLH")]',
    );
    await waitForVisible(
      `${xpath.dialog}[.//h2[normalize-space()="About OFLH"]]`,
    );
    await screenshot("about dialog");
    await click(`${xpath.dialog}//button[@aria-label="Close dialog"]`);
    await waitForGone(xpath.dialog);
  });

  // #192, fixed in #193 once the desktop Status reads build_commit().
  it("About shows the git commit of a local development build", async () => {
    await click('//nav//button[@aria-label="Settings"]');
    await click(
      '//*[@role="menuitem"][starts-with(normalize-space(.), "About OFLH")]',
    );
    await waitForVisible(
      `${xpath.dialog}[.//h2[normalize-space()="About OFLH"]]`,
    );
    const text = await $(xpath.dialog).getText();
    await screenshot("about shows commit");
    await click(`${xpath.dialog}//button[@aria-label="Close dialog"]`);
    await waitForGone(xpath.dialog);
    expect(text).toMatch(/\b[0-9a-f]{7,}\b/);
  });

  it("Donate explains the support options and closes", async () => {
    await click(xpath.titled("Choose how to support OFLH"));
    await waitForVisible(xpath.dialog);
    await screenshot("donate options");
    await click(`${xpath.dialog}//button[@aria-label="Close dialog"]`);
    await waitForGone(xpath.dialog);
  });

  it("the sidebar collapses and expands", async () => {
    await resetWorkspace();
    await clickButton("Collapse sidebar");
    await waitForVisible("nav.sidebar.collapsed");
    await screenshot("collapsed sidebar");
    await clickButton("Expand sidebar");
    await waitForGone("nav.sidebar.collapsed");
  });
});

/** The gear menu with German labels ("Einstellungen"). */
async function openSettingsGerman(): Promise<void> {
  await click('//nav//button[@aria-label="Einstellungen"]');
  await click(
    '//*[@role="menuitem"][starts-with(normalize-space(.), "Einstellungen")]',
  );
  await waitForVisible("#language");
}

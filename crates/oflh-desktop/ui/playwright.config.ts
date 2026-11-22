import { defineConfig } from "@playwright/test";

// Default to the Chrome channel used by the frontend job. PLAYWRIGHT_CHANNEL=chromium
// selects Playwright's bundled Chromium, which is available on Linux ARM64 where
// Google Chrome cannot be installed.
const channel = process.env.PLAYWRIGHT_CHANNEL ?? "chrome";

export default defineConfig({
  testDir: "./tests",
  reporter: [["list"], ["html", { open: "never" }]],
  forbidOnly: !!process.env.CI,
  use: {
    locale: "en-US",
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
    baseURL: "http://127.0.0.1:1420",
    viewport: { width: 1280, height: 800 },
    // Show real scrollbars (headless hides them) so the slim thumb is tested.
    launchOptions: {
      ...(channel === "chromium" ? {} : { channel }),
      ignoreDefaultArgs: ["--hide-scrollbars"],
    },
  },
  webServer: {
    command: "npm run dev",
    url: "http://127.0.0.1:1420",
    reuseExistingServer: !process.env.CI,
    env: { ...process.env, OFLH_PLAYWRIGHT: "1" },
  },
});

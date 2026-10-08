// Renders template.html to social-preview.png at exactly 1280x640 (deviceScaleFactor 1).
// Usage: node render.cjs [template.html] [out.png]
// Needs the `playwright` package (NODE_PATH or a local install) and its Chromium build.
const path = require("path");
const { chromium } = require("playwright");

(async () => {
  const tpl = path.resolve(process.argv[2] || path.join(__dirname, "template.html"));
  const out = path.resolve(process.argv[3] || path.join(__dirname, "..", "social-preview.png"));
  const browser = await chromium.launch();
  const page = await browser.newPage({ viewport: { width: 1280, height: 640 }, deviceScaleFactor: 1 });
  await page.goto("file://" + tpl, { waitUntil: "load" });
  await page.evaluate(() => document.fonts.ready);
  await page.waitForTimeout(150);
  await page.screenshot({ path: out, clip: { x: 0, y: 0, width: 1280, height: 640 } });
  await browser.close();
  console.log("wrote", out);
})().catch((e) => { console.error(e); process.exit(1); });

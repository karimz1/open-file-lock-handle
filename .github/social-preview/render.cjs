// Renders template.html to social-preview.png at exactly 1280x640 (deviceScaleFactor 1).
// Usage: node render.cjs [template.html] [out.png]
// Needs the `playwright` package (NODE_PATH or a local install) and its Chromium build.
// Exits non-zero if any asset fails to load, so a moved image or font can't slip through.
const path = require("path");
const { chromium } = require("playwright");

(async () => {
  const tpl = path.resolve(process.argv[2] || path.join(__dirname, "template.html"));
  const out = path.resolve(process.argv[3] || path.join(__dirname, "..", "social-preview.png"));
  const problems = [];
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage({ viewport: { width: 1280, height: 640 }, deviceScaleFactor: 1 });
    page.on("requestfailed", (r) => problems.push(`request failed: ${r.url()} (${r.failure()?.errorText})`));
    page.on("pageerror", (e) => problems.push(`page error: ${e.message}`));
    page.on("console", (m) => { if (m.type() === "error") problems.push(`console error: ${m.text()}`); });

    await page.goto("file://" + tpl, { waitUntil: "load" });
    await page.evaluate(() => document.fonts.ready);
    problems.push(...(await page.evaluate(() => [
      ...[...document.images]
        .filter((img) => !img.complete || img.naturalWidth === 0)
        .map((img) => `image did not load: ${img.getAttribute("src")}`),
      ...[...document.fonts]
        .filter((f) => f.status !== "loaded")
        .map((f) => `font ${f.family} is ${f.status}`),
    ])));
    if (problems.length) throw new Error("template did not render cleanly:\n  " + problems.join("\n  "));

    await page.waitForTimeout(150);
    await page.screenshot({ path: out, clip: { x: 0, y: 0, width: 1280, height: 640 } });
    console.log("wrote", out);
  } finally {
    await browser.close();
  }
})().catch((e) => { console.error(e.message || e); process.exit(1); });

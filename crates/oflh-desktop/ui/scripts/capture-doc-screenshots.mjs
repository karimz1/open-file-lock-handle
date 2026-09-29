import { spawnSync } from "node:child_process";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const uiDirectory = resolve(fileURLToPath(new URL("..", import.meta.url)));
const playwrightCli = resolve(uiDirectory, "node_modules/playwright/cli.js");
const result = spawnSync(
  process.execPath,
  [playwrightCli, "test", "--grep", "documentation screenshots"],
  {
    cwd: uiDirectory,
    env: { ...process.env, OFLH_UPDATE_SCREENSHOTS: "1" },
    stdio: "inherit",
  },
);

if (result.error) {
  console.error(result.error);
  process.exitCode = 1;
} else {
  process.exitCode = result.status ?? 1;
}

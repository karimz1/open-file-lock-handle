import path from "node:path";
import { fileURLToPath } from "node:url";
import { platformLabel } from "./platform.ts";

const here = path.dirname(fileURLToPath(import.meta.url));

/** The `gui-e2e` package directory. */
export const suiteRoot = path.resolve(here, "..");

/** Repository root (three levels above `crates/oflh-desktop/gui-e2e`). */
export const repositoryRoot = path.resolve(suiteRoot, "..", "..", "..");

/** Screenshots, app logs and the result summary of this run. */
export const artifactsDir = path.resolve(
  process.env.OFLH_E2E_ARTIFACTS ??
    path.join(suiteRoot, "artifacts", platformLabel),
);

export const screenshotsDir = path.join(artifactsDir, "screenshots");
export const resultsFile = path.join(artifactsDir, "results.jsonl");

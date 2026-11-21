import fs from "node:fs";
import path from "node:path";
import { artifactsDir, resultsFile } from "./paths.ts";
import {
  operatingSystem,
  platformLabel,
  type OperatingSystem,
} from "./platform.ts";

/**
 * Per-test outcomes of this run, written as JSON lines by the workers and
 * turned into a Markdown table (job summary and artifact) by the launcher.
 */
export type Outcome = "passed" | "failed" | "known failing" | "not applicable";

export interface TestResult {
  spec: string;
  title: string;
  outcome: Outcome;
  detail?: string;
}

export function recordResult(result: TestResult): void {
  fs.mkdirSync(path.dirname(resultsFile), { recursive: true });
  fs.appendFileSync(resultsFile, `${JSON.stringify(result)}\n`);
}

function errorDetail(error: unknown): string {
  return String(error)
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .slice(0, 6)
    .join(" — ")
    .slice(0, 400);
}

const knownFailingMarker = /\[known failing, #(\d+)\]$/;
const notApplicableMarker = /\[not applicable on \w+\]$/;

/**
 * A test for a bug that is reported but not fixed on this branch yet. It runs
 * like any other test; while it fails, the run stays green and the summary
 * lists it as known failing with its issue. Once it passes, it fails with a
 * reminder to turn it back into a plain `it`, so a fix cannot go unnoticed.
 */
export function itKnownFailing(
  issue: number,
  title: string,
  test: () => Promise<void>,
): void {
  const marked = `${title} [known failing, #${issue}]`;
  it(marked, async function () {
    try {
      await test.call(this);
    } catch (error) {
      const detail = errorDetail(error);
      knownFailures.set(marked, detail);
      console.log(`Known failing (#${issue}): ${detail}`);
      return;
    }
    throw new Error(
      `Issue #${issue} looks fixed: "${title}" passes now. Replace itKnownFailing with it.`,
    );
  });
}

/** First error line of each known failing test that failed as expected, for the summary. */
export const knownFailures = new Map<string, string>();

/**
 * A test of behavior that only exists on some systems (for example Windows
 * WM_CLOSE handling). Elsewhere it is reported as not applicable with `reason`,
 * in the summary and the job log, never silently left out.
 */
export function itOn(
  systems: OperatingSystem[],
  reason: string,
  title: string,
  test: () => Promise<void>,
): void {
  if (systems.includes(operatingSystem)) {
    it(title, test);
  } else {
    const marked = `${title} [not applicable on ${operatingSystem}]`;
    it(marked, function () {
      recordResult({
        spec: process.env.OFLH_E2E_SPEC ?? "spec",
        title: marked,
        outcome: "not applicable",
        detail: reason,
      });
      this.skip();
    });
  }
}

export function classify(
  title: string,
  passed: boolean,
  skipped: boolean,
): Outcome {
  if (notApplicableMarker.test(title) || skipped) return "not applicable";
  if (knownFailingMarker.test(title) && passed) return "known failing";
  return passed ? "passed" : "failed";
}

/** Build the Markdown summary from the recorded results. */
export function writeSummary(): { markdown: string; failed: number } {
  const results: TestResult[] = fs.existsSync(resultsFile)
    ? fs
        .readFileSync(resultsFile, "utf8")
        .split("\n")
        .filter(Boolean)
        .map((line) => JSON.parse(line) as TestResult)
    : [];
  const count = (outcome: Outcome) =>
    results.filter((result) => result.outcome === outcome).length;
  const failed = count("failed");
  const icon: Record<Outcome, string> = {
    passed: "pass",
    failed: "FAIL",
    "known failing": "known failing",
    "not applicable": "n/a",
  };
  const lines = [
    `### GUI end-to-end tests on ${platformLabel}: ${failed ? "failed" : "passed"}`,
    "",
    `${count("passed")} passed, ${failed} failed, ${count("known failing")} known failing, ${count("not applicable")} not applicable on this system.`,
    "",
    "| Spec | Interaction | Result | Detail |",
    "| --- | --- | --- | --- |",
    ...results.map(
      (result) =>
        `| ${result.spec} | ${result.title.replace(/\|/g, "\\|")} | ${icon[result.outcome]} | ${(result.detail ?? "").replace(/\|/g, "\\|").replace(/\n/g, " ")} |`,
    ),
    "",
  ];
  const markdown = lines.join("\n");
  fs.mkdirSync(artifactsDir, { recursive: true });
  fs.writeFileSync(path.join(artifactsDir, "summary.md"), markdown);
  fs.writeFileSync(
    path.join(artifactsDir, "results.json"),
    JSON.stringify(results, null, 2),
  );
  return { markdown, failed };
}

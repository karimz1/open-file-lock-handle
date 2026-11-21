// Fail a workflow's final status job unless every needed job, including each
// matrix leg, finished successfully. GitHub reports a matrix job as `success`
// only when all of its legs succeeded; `failure`, `cancelled`, and unexpected
// `skipped` results fail this check. Skips listed in ALLOWED_SKIPS are
// intentional (documented next to the job) and are reported, not hidden.
import { appendFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

export function evaluate(needs, allowedSkips = []) {
  const allowed = new Set(allowedSkips);
  const rows = Object.entries(needs).map(([job, value]) => {
    const result = value?.result ?? "missing";
    const ok =
      result === "success" || (result === "skipped" && allowed.has(job));
    return { job, result, ok, intentional: result === "skipped" && ok };
  });
  for (const job of allowed) {
    if (!(job in needs)) {
      throw new Error(`ALLOWED_SKIPS names ${job}, which is not in needs`);
    }
  }
  return { ok: rows.length > 0 && rows.every((row) => row.ok), rows };
}

export function summary(title, { ok, rows }) {
  const lines = [
    `### ${title}: ${ok ? "passed" : "failed"}`,
    "",
    "| Job | Result |",
    "| --- | --- |",
    ...rows.map(
      ({ job, result, ok, intentional }) =>
        `| ${job} | ${result}${intentional ? " (intentional skip)" : ok ? "" : " (blocking)"} |`,
    ),
  ];
  return lines.join("\n") + "\n";
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  const title = process.env.RESULT_TITLE || "Workflow result";
  const needs = JSON.parse(process.env.NEEDS || "{}");
  const allowed = (process.env.ALLOWED_SKIPS || "").split(/\s+/).filter(Boolean);
  const outcome = evaluate(needs, allowed);
  const text = summary(title, outcome);
  process.stdout.write(text);
  if (process.env.GITHUB_STEP_SUMMARY) {
    appendFileSync(process.env.GITHUB_STEP_SUMMARY, text);
  }
  for (const { job, result, ok } of outcome.rows) {
    if (!ok) {
      console.log(`::error title=${title}::${job} finished as ${result}`);
    }
  }
  if (!outcome.ok) process.exit(1);
}

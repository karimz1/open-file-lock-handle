import assert from "node:assert/strict";
import { test } from "node:test";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { evaluate, summary } from "./require-success.mjs";

const script = fileURLToPath(new URL("./require-success.mjs", import.meta.url));
const run = (needs, allowed = "") => {
  try {
    execFileSync(process.execPath, [script], {
      env: { ...process.env, NEEDS: JSON.stringify(needs), ALLOWED_SKIPS: allowed, GITHUB_STEP_SUMMARY: "" },
      stdio: "pipe",
    });
    return 0;
  } catch (error) {
    return error.status;
  }
};

test("every job and matrix leg succeeded", () => {
  assert.equal(evaluate({ test: { result: "success" }, lint: { result: "success" } }).ok, true);
  assert.equal(run({ test: { result: "success" } }), 0);
});

test("one failed matrix leg fails the aggregate", () => {
  // GitHub reports the whole matrix job as failure when any leg fails.
  const needs = { lockfile: { result: "success" }, test: { result: "failure" } };
  assert.equal(evaluate(needs).ok, false);
  assert.equal(run(needs), 1);
});

test("cancelled and unexpected skipped results fail", () => {
  assert.equal(run({ test: { result: "cancelled" } }), 1);
  assert.equal(run({ summary: { result: "skipped" } }), 1);
  assert.equal(run({}), 1);
});

test("only documented skips pass and stay visible", () => {
  const needs = { native: { result: "success" }, prune: { result: "skipped" } };
  assert.equal(run(needs, "prune"), 0);
  assert.match(summary("Desktop result", evaluate(needs, ["prune"])), /prune \| skipped \(intentional skip\)/);
  assert.equal(run({ native: { result: "failure" }, prune: { result: "skipped" } }, "prune"), 1);
  assert.equal(run({ prune: { result: "failure" } }, "prune"), 1);
  assert.throws(() => evaluate(needs, ["typo"]));
});

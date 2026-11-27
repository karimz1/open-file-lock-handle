import assert from "node:assert/strict";
import test from "node:test";
import {
  explorerFolderFromTitle,
  explorerFolderIsOpen,
  selectionGraceDecision,
  type SelectionGrace,
} from "./explorer-reveal.ts";

const folder =
  "C:\\Users\\runneradmin\\AppData\\Local\\Temp\\oflh-e2e-details-f80WZu\\target";

test("strips the File Explorer title suffix", () => {
  assert.equal(explorerFolderFromTitle("target - File Explorer"), "target");
  assert.equal(explorerFolderFromTitle("target"), "target");
});

test("a folder window is open when only the Explorer title is present", () => {
  assert.equal(
    explorerFolderIsOpen(
      [],
      ["OFLH Desktop", "target - File Explorer"],
      folder,
    ),
    true,
  );
});

test("a folder window is open when Shell.Application reports the folder", () => {
  assert.equal(
    explorerFolderIsOpen([{ folder }], ["OFLH Desktop"], folder),
    true,
  );
});

test("a folder window is not assumed open from unrelated titles", () => {
  assert.equal(
    explorerFolderIsOpen(
      [],
      [
        "OFLH Desktop",
        "C:\\ProgramData\\GitHub\\HostedComputeAgent\\hosted-compute-agent",
      ],
      folder,
    ),
    false,
  );
});

test("grace accepts after the folder has stayed open for the grace period", () => {
  const grace: SelectionGrace = { seenSince: 0 };
  assert.equal(selectionGraceDecision(true, 1000, grace, 8000), "waiting");
  assert.equal(selectionGraceDecision(true, 8999, grace, 8000), "waiting");
  assert.equal(selectionGraceDecision(true, 9000, grace, 8000), "accept");
});

test("a combined miss resets the grace period", () => {
  const grace: SelectionGrace = { seenSince: 0 };
  assert.equal(selectionGraceDecision(true, 1000, grace, 8000), "waiting");
  assert.equal(selectionGraceDecision(false, 5000, grace, 8000), "not-open");
  assert.equal(selectionGraceDecision(true, 5000, grace, 8000), "waiting");
  assert.equal(selectionGraceDecision(true, 12999, grace, 8000), "waiting");
  assert.equal(selectionGraceDecision(true, 13000, grace, 8000), "accept");
});

test("a first-probe miss must not reset grace when another probe still sees the folder", () => {
  // CI failure on e0507f9, job 140639528052: Shell.Application and raw
  // process titles sometimes missed the window, while UI Automation still
  // showed "target - File Explorer". Updating the timer from the first
  // probe alone reset the 8s grace period and the 20s wait ran out.
  const grace: SelectionGrace = { seenSince: 0 };
  const shellMiss = explorerFolderIsOpen([], ["OFLH Desktop"], folder, false);
  const combined = explorerFolderIsOpen([], ["OFLH Desktop"], folder, true);
  assert.equal(shellMiss, false);
  assert.equal(combined, true);
  assert.equal(selectionGraceDecision(combined, 1000, grace, 8000), "waiting");
  assert.equal(selectionGraceDecision(combined, 9000, grace, 8000), "accept");

  const wrong: SelectionGrace = { seenSince: 0 };
  selectionGraceDecision(true, 1000, wrong, 8000);
  selectionGraceDecision(shellMiss, 5000, wrong, 8000);
  selectionGraceDecision(combined, 5000, wrong, 8000);
  assert.equal(selectionGraceDecision(combined, 9000, wrong, 8000), "waiting");
});

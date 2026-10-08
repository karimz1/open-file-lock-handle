import { describe, expect, it } from "vitest";
import { normalizeTypedPath, showsTargetHint } from "./targetPath";

describe("typed target paths", () => {
  it("removes surrounding whitespace", () => {
    expect(normalizeTypedPath("  /workspace/project\n")).toBe(
      "/workspace/project",
    );
  });

  it("removes one pair of matching outer quotes from copied paths", () => {
    expect(normalizeTypedPath('"C:\\Users\\alice\\report.docx"')).toBe(
      "C:\\Users\\alice\\report.docx",
    );
    expect(normalizeTypedPath("'/workspace/my project/'")).toBe(
      "/workspace/my project/",
    );
    expect(normalizeTypedPath(' "D:\\Shared Folder\\" ')).toBe(
      "D:\\Shared Folder\\",
    );
  });

  it("keeps quotes that are part of the name or unmatched", () => {
    expect(normalizeTypedPath('/workspace/"draft".txt')).toBe(
      '/workspace/"draft".txt',
    );
    expect(normalizeTypedPath("\"/workspace/file'")).toBe("\"/workspace/file'");
    expect(normalizeTypedPath('"')).toBe('"');
  });

  it("returns an empty string for blank or empty quoted input", () => {
    expect(normalizeTypedPath("   ")).toBe("");
    expect(normalizeTypedPath('""')).toBe("");
  });
});

describe("drag-and-drop hint visibility", () => {
  it("shows the hint in the default window at the default font size", () => {
    expect(showsTargetHint(800, 14)).toBe(true);
  });

  it("hides the hint in short windows or with large fonts", () => {
    expect(showsTargetHint(560, 14)).toBe(false);
    expect(showsTargetHint(800, 24)).toBe(false);
    expect(showsTargetHint(1200, 24)).toBe(true);
  });
});

import { describe, expect, it } from "vitest";
import { normalizeTypedPath } from "./targetPath";

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

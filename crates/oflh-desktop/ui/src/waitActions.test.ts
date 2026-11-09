import { describe, expect, it } from "vitest";
import {
  validWaitName,
  waitDestinationPreview,
  waitPathParts,
} from "./waitActions";
describe("waiting file follow-ups", () => {
  it("accepts Unicode components and rejects paths, controls and device names", () => {
    for (const name of [
      "Bestätigung neu.pdf",
      "文件.pdf",
      ".hidden",
      "archive.tar.gz",
    ])
      expect(validWaitName(name)).toBe(true);
    for (const name of [
      "",
      " ",
      ".",
      "..",
      "../escape",
      "folder/file",
      "folder\\file",
      "C:\\file.pdf",
      "bad\0.pdf",
      "bad\u0085.pdf",
      "CON",
      "nul.pdf",
      "COM1.txt",
      "LPT9",
      "bad?.pdf",
      "bad.",
      "bad ",
    ])
      expect(validWaitName(name)).toBe(false);
  });
  it("previews paths without interpreting Unix backslashes as separators", () => {
    expect(
      waitDestinationPreview("C:\\Destination", "C:\\Source\\Bestätigung.pdf"),
    ).toBe("C:\\Destination\\Bestätigung.pdf");
    expect(waitDestinationPreview("C:\\", "C:\\Source\\Bestätigung.pdf")).toBe(
      "C:\\Bestätigung.pdf",
    );
    expect(
      waitDestinationPreview("/build/destination/", "/build/source/a\\b.pdf"),
    ).toBe("/build/destination/a\\b.pdf");
    expect(waitDestinationPreview("/", "/build/source.pdf")).toBe(
      "/source.pdf",
    );
    expect(waitPathParts("\\\\server\\share\\file.pdf").parent).toBe(
      "\\\\server\\share\\",
    );
  });
});

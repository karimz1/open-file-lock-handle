import { describe, expect, it } from "vitest";
import {
  resolveWaitNewName,
  validWaitName,
  waitDestinationPreview,
  waitNameParts,
  waitNameSelectionRange,
  waitPathParts,
  waitRenamePreview,
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

describe("wait rename name and extension", () => {
  it("treats the suffix after the last dot as the extension, like Explorer and Finder", () => {
    expect(waitNameParts("report.pdf")).toEqual({
      base: "report",
      extension: ".pdf",
    });
    expect(waitNameParts("archive.tar.gz")).toEqual({
      base: "archive.tar",
      extension: ".gz",
    });
    expect(waitNameParts("Bestellbestätigung .pdf")).toEqual({
      base: "Bestellbestätigung ",
      extension: ".pdf",
    });
    expect(waitNameParts("文件.PDF")).toEqual({
      base: "文件",
      extension: ".PDF",
    });
  });

  it("treats dotfiles and extensionless names as a base only", () => {
    expect(waitNameParts(".gitignore")).toEqual({
      base: ".gitignore",
      extension: "",
    });
    expect(waitNameParts(".env.local")).toEqual({
      base: ".env",
      extension: ".local",
    });
    expect(waitNameParts("Makefile")).toEqual({
      base: "Makefile",
      extension: "",
    });
    expect(waitNameParts("README")).toEqual({
      base: "README",
      extension: "",
    });
  });

  it("does not split extensions on folder names", () => {
    expect(waitNameParts("backup.old", { folder: true })).toEqual({
      base: "backup.old",
      extension: "",
    });
    expect(waitNameParts("My.Documents", { folder: true })).toEqual({
      base: "My.Documents",
      extension: "",
    });
    expect(waitNameParts(".config", { folder: true })).toEqual({
      base: ".config",
      extension: "",
    });
  });

  it("selects only the base name so typing keeps the last extension", () => {
    expect(waitNameSelectionRange("report.pdf")).toEqual([0, 6]);
    expect(waitNameSelectionRange("archive.tar.gz")).toEqual([0, 11]);
    expect(waitNameSelectionRange("Bestellbestätigung .pdf")).toEqual([
      0,
      "Bestellbestätigung ".length,
    ]);
    expect(waitNameSelectionRange(".gitignore")).toEqual([
      0,
      ".gitignore".length,
    ]);
    expect(waitNameSelectionRange("Makefile")).toEqual([0, "Makefile".length]);
    expect(waitNameSelectionRange("backup.old", { folder: true })).toEqual([
      0,
      "backup.old".length,
    ]);
  });

  it("keeps the original extension when the typed name omits one", () => {
    expect(resolveWaitNewName("report.pdf", "invoice")).toBe("invoice.pdf");
    expect(resolveWaitNewName("report.pdf", "invoice.pdf")).toBe("invoice.pdf");
    expect(resolveWaitNewName("Bestellbestätigung .pdf", "Archiv")).toBe(
      "Archiv.pdf",
    );
    expect(resolveWaitNewName("archive.tar.gz", "backup")).toBe("backup.gz");
    expect(resolveWaitNewName("文件.PDF", "副本")).toBe("副本.PDF");
  });

  it("keeps a different extension only when the user typed one", () => {
    expect(resolveWaitNewName("report.pdf", "invoice.docx")).toBe(
      "invoice.docx",
    );
    expect(resolveWaitNewName("report.pdf", "invoice.PDF")).toBe("invoice.PDF");
    expect(resolveWaitNewName("report.pdf", "archive.tar.gz")).toBe(
      "archive.tar.gz",
    );
  });

  it("does not invent an extension for dotfiles, extensionless files, or folders", () => {
    expect(resolveWaitNewName(".gitignore", "npmrc")).toBe("npmrc");
    expect(resolveWaitNewName(".gitignore", ".npmrc")).toBe(".npmrc");
    expect(resolveWaitNewName("Makefile", "GNUmakefile")).toBe("GNUmakefile");
    expect(resolveWaitNewName("report.pdf", ".gitignore")).toBe(".gitignore");
    expect(resolveWaitNewName("backup.old", "archive", { folder: true })).toBe(
      "archive",
    );
    expect(
      resolveWaitNewName("My.Documents", "Work.Archive", { folder: true }),
    ).toBe("Work.Archive");
  });

  it("previews the resolved name and path so a restored extension is visible", () => {
    expect(
      waitRenamePreview("/build/documents/", "report.pdf", "invoice"),
    ).toEqual({
      name: "invoice.pdf",
      path: "/build/documents/invoice.pdf",
      extensionRestored: true,
    });
    expect(
      waitRenamePreview("/build/documents/", "report.pdf", "invoice.docx"),
    ).toEqual({
      name: "invoice.docx",
      path: "/build/documents/invoice.docx",
      extensionRestored: false,
    });
    expect(
      waitRenamePreview("C:\\Documents\\", "report.pdf", "invoice"),
    ).toEqual({
      name: "invoice.pdf",
      path: "C:\\Documents\\invoice.pdf",
      extensionRestored: true,
    });
    expect(
      waitRenamePreview("/build/docs/", "backup.old", "archive", {
        folder: true,
      }),
    ).toEqual({
      name: "archive",
      path: "/build/docs/archive",
      extensionRestored: false,
    });
  });
});

import { describe, expect, it } from "vitest";
import { pageKind, staleRowsFor, virtualRowKey } from "./gridPages";

const page = {
  target: "/work",
  kind: pageKind({ ports: false, handles: false }),
  queryKey: "a",
};
const wanted = { target: "/work", kind: page.kind, queryKey: "b" };

describe("staleRowsFor", () => {
  it("keeps the previous page of the same view and target for a new query", () => {
    expect(staleRowsFor(page, wanted, false)).toBe(page);
  });
  it("has nothing to keep before the first page", () => {
    expect(staleRowsFor(null, wanted, false)).toBeNull();
  });
  it("is not stale when the query did not change", () => {
    expect(staleRowsFor(page, { ...wanted, queryKey: "a" }, false)).toBeNull();
  });
  it("never carries rows over to another target", () => {
    expect(
      staleRowsFor(page, { ...wanted, target: "/other" }, false),
    ).toBeNull();
  });
  it("never shows process rows under port or file usage columns", () => {
    for (const next of [
      pageKind({ ports: true, handles: false }),
      pageKind({ ports: false, handles: true }),
    ])
      expect(staleRowsFor(page, { ...wanted, kind: next }, false)).toBeNull();
  });
  it("drops the old rows once the new page failed", () => {
    expect(staleRowsFor(page, wanted, true)).toBeNull();
  });
});

describe("virtualRowKey", () => {
  const row = { key: "10:1:0/exe", matching_files: null };
  it("does not depend on the query, so a refilter can reuse the row", () => {
    expect(virtualRowKey("14:", row, 3)).toBe(virtualRowKey("14:", row, 90));
  });
  it("changes with the font size or columns, which change the row height", () => {
    expect(virtualRowKey("14:", row, 3)).not.toBe(virtualRowKey("15:", row, 3));
  });
  it("changes when the matching file paths change, which can wrap differently", () => {
    const matching = (display: string) => ({
      key: row.key,
      matching_files: {
        total: 2,
        files: [{ path: { display, reference: "r" } }],
      },
    });
    expect(virtualRowKey("14:", matching("/a/short"), 3)).not.toBe(
      virtualRowKey("14:", matching("/a/a-much-longer/path/that/wraps"), 3),
    );
  });
  it("falls back to the index for rows that have not loaded yet", () => {
    expect(virtualRowKey("14:", undefined, 7)).toBe("14::7");
  });
});

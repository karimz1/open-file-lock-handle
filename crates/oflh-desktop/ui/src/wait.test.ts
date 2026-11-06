import { describe, expect, it } from "vitest";
import { elapsedLabel, waitTarget } from "./WaitUntilFree";

describe("wait target and timeout display", () => {
  it("waits for the whole filtered port, not a single process", () => {
    expect(waitTarget("unused", "port:3000")).toEqual({
      path: null,
      port: 3000,
    });
    expect(waitTarget("", "node port:3000")).toEqual({
      path: null,
      port: 3000,
    });
  });
  it("retains file paths and rejects an invalid port", () => {
    expect(waitTarget(" /tmp/file ", "")).toEqual({
      path: "/tmp/file",
      port: null,
    });
    expect(waitTarget("/tmp/file", "port:0")).toEqual({
      path: null,
      port: null,
    });
    expect(waitTarget("/tmp/held", "port:99999")).toEqual({
      path: null,
      port: null,
    });
  });
  it("fits elapsed time into a compact status line", () => {
    expect(elapsedLabel(0)).toBe("0:00");
    expect(elapsedLabel(61_999)).toBe("1:01");
  });
});

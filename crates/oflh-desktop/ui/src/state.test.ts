import { describe, it, expect } from "vitest";
import {
  acceptStatus,
  initialStatus,
  selectKey,
  memory,
  compactPath,
} from "./state";
describe("desktop state", () => {
  it("rejects obsolete completions and late scan acknowledgments", () => {
    const scanning = { ...initialStatus, generation: 2, scanning: true };
    expect(acceptStatus(scanning, { ...initialStatus, generation: 1 })).toBe(
      scanning,
    );
    const done = { ...scanning, scanning: false, revision: 2 };
    expect(acceptStatus(done, scanning)).toBe(done);
  });
  it("never aliases different process lifetimes or truncates birth counters", () => {
    const first = "32:18446744073709551614:0";
    const second = "32:18446744073709551615:0";
    expect([...selectKey(new Set([first]), second, true)]).toEqual([
      first,
      second,
    ]);
    expect([...selectKey(new Set([first, second]), first, true)]).toEqual([
      second,
    ]);
  });
  it("keeps unknown memory distinct from zero", () => {
    expect(memory(null)).toBe("—");
    expect(memory(0)).toBe("0.0 MB");
  });
});

describe("compact display paths", () => {
  it("uses target boundaries and preserves full-path clipboard inputs", () => {
    expect(compactPath("/work/app/bin/test.dll", "/work/app")).toBe(
      "bin/test.dll",
    );
    expect(compactPath("/work/application/test.dll", "/work/app")).toBe(
      "test.dll",
    );
    expect(compactPath("/work/test.dll", "/work/test.dll")).toBe("test.dll");
    expect(compactPath("/work/test.dll", "/")).toBe("work/test.dll");
    expect(compactPath("", "")).toBe("");
  });
  it("handles Windows drive and UNC displays without splitting Unix backslashes", () => {
    expect(
      compactPath(String.raw`C:\work\bin\test.dll`, String.raw`C:\work`),
    ).toBe("bin/test.dll");
    expect(
      compactPath(
        String.raw`\\host\share\bin\test.dll`,
        String.raw`\\host\share`,
      ),
    ).toBe("bin/test.dll");
    expect(compactPath(String.raw`/work/file\name`, "/elsewhere")).toBe(
      String.raw`file\name`,
    );
  });
});

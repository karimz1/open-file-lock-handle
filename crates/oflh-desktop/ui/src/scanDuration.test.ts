import { describe, expect, it } from "vitest";
import { scanDuration } from "./scanDuration";

describe("completed inspection durations", () => {
  it.each([
    [0, "0 ms"],
    [999, "999 ms"],
    [1000, "1 sec"],
    [1532, "1.5 sec"],
    [60000, "1 min"],
    [90000, "1.5 min"],
    [7200000, "120 min"],
  ])("formats %i milliseconds as %s", (milliseconds, expected) => {
    expect(scanDuration(milliseconds, "en")).toBe(expected);
  });
  it("uses localized units and decimal separators", () => {
    expect(scanDuration(1532, "de")).toBe("1,5 Sek.");
    expect(scanDuration(90000, "de")).toBe("1,5 Min.");
    expect(scanDuration(1532, "zh")).toBe("1.5秒");
    expect(scanDuration(23, "zh")).toBe("23毫秒");
  });
});

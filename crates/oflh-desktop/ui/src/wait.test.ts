import { describe, expect, it } from "vitest";
import { elapsedLabel } from "./WaitUntilFree";

describe("wait target and timeout display", () => {
  it("fits elapsed time into a compact status line", () => {
    expect(elapsedLabel(0)).toBe("0:00");
    expect(elapsedLabel(61_999)).toBe("1:01");
  });
});

import { describe, expect, it } from "vitest";
import { formatVersionLabel, shortCommit } from "./buildInfo";

describe("build info labels", () => {
  it("abbreviates full and dirty commits", () => {
    expect(shortCommit("")).toBe("");
    expect(shortCommit("abc1234")).toBe("abc1234");
    expect(shortCommit("abc1234-dirty")).toBe("abc1234-dirty");
    expect(shortCommit("0123456789abcdef0123456789abcdef01234567")).toBe(
      "0123456",
    );
    expect(shortCommit("0123456789abcdef0123456789abcdef01234567-dirty")).toBe(
      "0123456-dirty",
    );
  });

  it("renders the compact About version label", () => {
    expect(formatVersionLabel("0.9.0-rc.1", "")).toBe("0.9.0-rc.1");
    expect(formatVersionLabel("0.9.0-rc.1", "abc1234")).toBe(
      "0.9.0-rc.1 (abc1234)",
    );
    expect(
      formatVersionLabel(
        "0.9.0-rc.1",
        "0123456789abcdef0123456789abcdef01234567-dirty",
      ),
    ).toBe("0.9.0-rc.1 (0123456-dirty)");
  });
});

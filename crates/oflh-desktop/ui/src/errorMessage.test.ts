import { describe, expect, it } from "vitest";
import { errorMessage } from "./api";
import en from "./locales/en";

describe("errorMessage", () => {
  it("translates failure categories the user can act on", () => {
    expect(
      errorMessage({
        kind: "protected_delete_target",
        message: "OFLH does not delete drive roots",
      }),
    ).toBe(en.wait.k_protected_delete);
    expect(errorMessage({ kind: "move_unsupported", message: "EINVAL" })).toBe(
      en.wait.k_move_unsupported,
    );
  });
  it("keeps native messages for other failures", () => {
    expect(errorMessage({ kind: "io", message: "Disk full" })).toBe(
      "Disk full",
    );
    expect(errorMessage("plain")).toBe("plain");
  });
});

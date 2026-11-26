import { describe, expect, it } from "vitest";
import { fileUsageNotice, fileUsageNoticeKeys } from "./coverageNotice";
import { translate, type MessageKey } from "./i18n";

describe("file usage coverage notice", () => {
  it("keeps the lock caveat on every platform and the Windows sentence only on Windows", () => {
    expect(fileUsageNoticeKeys("linux")).toEqual([
      "app.k_file_usage_is_not_proof_of_a_lock",
    ]);
    expect(fileUsageNoticeKeys("macos")).toEqual([
      "app.k_file_usage_is_not_proof_of_a_lock",
    ]);
    expect(fileUsageNoticeKeys("")).toEqual([
      "app.k_file_usage_is_not_proof_of_a_lock",
    ]);
    expect(fileUsageNoticeKeys("windows")).toEqual([
      "app.k_file_usage_is_not_proof_of_a_lock",
      "app.k_windows_resource_users_are_not_proven_lock_owners",
    ]);
  });

  it("joins the English sentences only on Windows", () => {
    const tr = (key: MessageKey) => translate("en", key);
    expect(fileUsageNotice("linux", tr)).toBe(
      "File usage is not proof of a lock.",
    );
    expect(fileUsageNotice("windows", tr)).toBe(
      "File usage is not proof of a lock. Windows resource users are not proven lock owners.",
    );
  });
});

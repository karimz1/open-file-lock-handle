import type { MessageKey } from "./i18n";

/** Status-bar keys for a file or folder inspection. The Windows clause is only about Restart Manager / resource usage. */
export function fileUsageNoticeKeys(os: string): MessageKey[] {
  const keys: MessageKey[] = ["app.k_file_usage_is_not_proof_of_a_lock"];
  if (os === "windows") {
    keys.push("app.k_windows_resource_users_are_not_proven_lock_owners");
  }
  return keys;
}

export function fileUsageNotice(
  os: string,
  tr: (key: MessageKey) => string,
): string {
  return fileUsageNoticeKeys(os).map(tr).join(" ");
}

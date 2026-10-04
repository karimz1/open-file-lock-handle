import { invoke } from "@tauri-apps/api/core";
export interface Failure {
  kind: string;
  message: string;
  os_code: number | null;
  details?: string | null;
}
export interface Status {
  generation: number;
  revision: number;
  scanning: boolean;
  target: string;
  processes: number;
  ports: number;
  usages: number;
  warnings: string[];
  error: Failure | null;
  version: string;
  commit: string;
  build_url: string;
  pull_request_url: string;
}
export interface Row {
  key: string;
  process_key: string;
  name: string;
  pid: number;
  user: string;
  path: string;
  path_ref: string;
  relation: string;
  access: string;
  evidence: string | null;
  evidence_label: string | null;
  deleted: boolean;
  memory: number | null;
  cpu: number | null;
  usages: number;
  actionable: boolean;
  port: {
    protocol: string;
    state: string;
    address: string;
    number: number;
    endpoint: string;
    reference: string;
  } | null;
}
export type Sort =
  | "relevance"
  | "name"
  | "pid"
  | "path"
  | "memory"
  | "cpu"
  | "port"
  | "protocol"
  | "address";
export interface ColumnFilters {
  name?: string;
  pid?: number;
  path?: string;
  cpu_min?: number;
  cpu_max?: number;
  memory_min?: number;
  memory_max?: number;
  evidence?: "any" | "present" | "kernel" | "sharing" | "none";
  access?: string;
}
export interface TableQuery {
  columns?: ColumnFilters;
  text: string;
  sort: Sort;
  descending: boolean;
  handles: boolean;
  ports: boolean;
  ports_path_only: boolean;
  locks_only: boolean;
  offset: number;
  limit: number;
  process_key?: string;
}
export interface Page {
  revision: number;
  total: number;
  rows: Row[];
}
export interface PathValue {
  display: string;
  reference: string;
}
export interface Details {
  process: Row;
  ports: number;
  can_inspect_folder: boolean;
  executable: PathValue;
  cwd: PathValue;
  ancestors: { name: string; pid: number; key: string; actionable: boolean }[];
}
export interface Confirmation {
  ticket: string;
  force: boolean;
  elevated: boolean;
  targets: { key: string; name: string; pid: number }[];
}
export interface ActionResult {
  outcome: "exited" | "still_running" | "unverified" | "failed";
  pid: number;
  admin_recovery?: boolean;
  error: Failure | null;
}
export const api = {
  systemInfo: () => invoke<{ os: string; arch: string }>("system_info"),
  updateMode: () => invoke<"download" | "install">("update_mode"),
  openDownload: () => invoke<void>("open_download"),
  openInstalledRelease: () => invoke<void>("open_installed_release"),
  openBuild: () => invoke<void>("open_build"),
  openPullRequest: () => invoke<void>("open_pull_request"),
  donate: () => invoke<void>("open_donation"),
  openPaypal: () => invoke<void>("open_paypal"),
  openSponsors: () => invoke<void>("open_sponsors"),
  openProfile: () => invoke<void>("open_profile"),
  openIssue: (title: string, body: string) =>
    invoke<void>("open_issue", { title, body }),
  openProject: () => invoke<void>("open_project"),
  openReleaseNotes: () => invoke<void>("open_release_notes"),
  status: () => invoke<Status>("status"),
  inspect: (path: string) => invoke<Status>("inspect", { path }),
  ports: () => invoke<Status>("inspect_ports"),
  followProcess: (revision: number, key: string) =>
    invoke<Status>("follow_process", { revision, key }),
  refresh: () => invoke<Status>("refresh"),
  cancel: () => invoke<Status>("cancel"),
  choose: (folder: boolean) => invoke<Status | null>("choose", { folder }),
  page: (revision: number, query: TableQuery) =>
    invoke<Page>("page", { revision, query }),
  details: (revision: number, key: string) =>
    invoke<Details>("details", { revision, key }),
  keys: (revision: number, query: TableQuery) =>
    invoke<string[]>("select_all", { revision, query }),
  copy: (revision: number, keys: string[], field: string, reference?: string) =>
    invoke<void>("copy", { revision, keys, field, reference }),
  copyDiagnostic: (text: string) => invoke<void>("copy_diagnostic", { text }),
  reveal: (revision: number, reference: string, containing: boolean) =>
    invoke<void>("reveal", { revision, reference, containing }),
  prepare: (revision: number, keys: string[], force: boolean) =>
    invoke<Confirmation>("prepare", { revision, keys, force }),
  prepareAncestor: (owner: string, key: string, force: boolean) =>
    invoke<Confirmation>("prepare_ancestor", { owner, key, force }),
  dismiss: () => invoke<void>("dismiss"),
  prepareElevated: (ticket: string) =>
    invoke<Confirmation>("prepare_elevated", { ticket }),
  terminate: (ticket: string) =>
    invoke<ActionResult[]>("terminate", { ticket }),
  recent: () => invoke<{ id: number; display: string }[]>("recent"),
  revisit: (id: number) => invoke<Status>("revisit", { id }),
  removeRecent: (id: number) => invoke<void>("remove_recent", { id }),
  clearRecent: () => invoke<void>("clear_recent"),
};
export function errorMessage(error: unknown): string {
  if (typeof error === "object" && error !== null && "message" in error)
    return String(error.message);
  return String(error);
}

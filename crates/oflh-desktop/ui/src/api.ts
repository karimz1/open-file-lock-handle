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
  elapsed_ms: number;
  last_scan_elapsed_ms: number | null;
  progress: {
    phase: "processes" | "files" | "ports" | "indexing";
    processes: number;
    resources: number;
    files: number;
    directories: number;
  };
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
  /** Opaque executable reference for the row's app icon. */
  icon_ref?: string;
  /** Login session id, filled for visible rows; null when unknown. */
  session?: string | null;
  session_kind?: SessionKind | null;
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
export type SessionKind =
  "console" | "rdp" | "ssh" | "tty" | "x11" | "wayland" | "services" | "other";
export type SessionState =
  "active" | "idle" | "disconnected" | "closing" | "other";
export type Availability =
  "available" | "requires_admin" | "requires_root" | "unsupported";
export interface SessionView {
  id: string;
  user: string;
  domain: string | null;
  kind: SessionKind;
  state: SessionState;
  client: string | null;
  tty: string | null;
  /** Logon time, Unix seconds. */
  since: number | null;
  own: boolean;
}
export interface OwnerView {
  user: string;
  service: boolean;
  session: SessionView | null;
}
export interface SessionActions {
  send_message: Availability;
  sign_out: Availability;
}
export interface SignOutConfirmation {
  ticket: string;
  session: SessionView;
  processes: CappedList;
}
export interface ShareFileView {
  id: number;
  path: string;
  user: string;
  client: string | null;
  locks: number;
  write: boolean;
}
export interface ShareFiles {
  close: Availability;
  files: ShareFileView[];
  error: string | null;
}
export interface CloseFileConfirmation {
  ticket: string;
  file: ShareFileView;
}
export interface Details {
  owner?: OwnerView;
  session_actions?: SessionActions | null;
  process: Row;
  ports: number;
  can_inspect_folder: boolean;
  executable: PathValue;
  cwd: PathValue;
  ancestors: { name: string; pid: number; key: string; actionable: boolean }[];
}
/** A list capped in Rust; `more` counts entries left out. */
export interface CappedList {
  items: string[];
  more: number;
}
/** Best-effort side effects; `null` means the platform could not read them. */
export interface TargetImpact {
  windows: CappedList | null;
  children: CappedList | null;
  files: CappedList;
  ports: CappedList;
  service?: string | null;
  critical?: boolean;
}
export interface ConfirmationTarget {
  key: string;
  name: string;
  pid: number;
  role?: "holder" | "ancestor";
  impact?: TargetImpact;
  owner?: OwnerView;
}
export interface Confirmation {
  ticket: string;
  force: boolean;
  elevated: boolean;
  targets: ConfirmationTarget[];
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
  processIcon: (revision: number, reference: string) =>
    invoke<string | null>("process_icon", { revision, reference }),
  inspect: (path: string) => invoke<Status>("inspect", { path }),
  inspectDropped: (request: number) =>
    invoke<Status>("inspect_dropped", { request }),
  newWindow: (request?: number) =>
    invoke<void>("new_window", { request: request ?? null }),
  ports: () => invoke<Status>("inspect_ports"),
  followProcess: (revision: number, key: string) =>
    invoke<Status>("follow_process", { revision, key }),
  refresh: () => invoke<Status>("refresh"),
  cancel: () => invoke<Status>("cancel"),
  closeInspection: () => invoke<Status>("close_inspection"),
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
  prepareSignOut: (revision: number, key: string) =>
    invoke<SignOutConfirmation>("prepare_sign_out", { revision, key }),
  signOut: (ticket: string) => invoke<void>("sign_out", { ticket }),
  sendMessage: (revision: number, key: string, text: string) =>
    invoke<void>("send_message", { revision, key, text }),
  shareFiles: () => invoke<ShareFiles>("share_files"),
  prepareCloseFile: (id: number) =>
    invoke<CloseFileConfirmation>("prepare_close_file", { id }),
  closeFile: (ticket: string) => invoke<void>("close_file", { ticket }),
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

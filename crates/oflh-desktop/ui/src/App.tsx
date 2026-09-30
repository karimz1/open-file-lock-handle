import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  Activity,
  ChevronDown,
  Columns3,
  SlidersHorizontal,
  Star,
  LoaderCircle,
  ArrowRight,
  Check,
  Copy,
  Coffee,
  ExternalLink,
  File,
  FileSearch,
  Files,
  FolderOpen,
  History,
  Info,
  Keyboard,
  Network,
  RefreshCw,
  Search,
  Settings,
  ShieldAlert,
  Timer,
  Trash2,
  X,
} from "lucide-react";
import {
  api,
  errorMessage,
  type ActionResult,
  type Confirmation,
  type Details,
  type Failure,
  type Row,
  type Sort,
  type Status,
  type TableQuery,
  type ColumnFilters,
} from "./api";
import { acceptStatus, initialStatus, selectKey } from "./state";
import { Inspector } from "./Inspector";
import { Modal } from "./Modal";
import { readTheme, ThemePicker } from "./Themes";
import {
  Table,
  fileColumns as fileColumnDefinitions,
  portColumns as portColumnDefinitions,
  type ColumnKey,
} from "./Table";
import { ColumnFilterPanel } from "./ColumnFilters";
import { t } from "./i18n";

type View = "ports" | "processes" | "handles" | "history" | "settings";
const columnKeys = new Set<ColumnKey>([
  "process",
  "pid",
  "path",
  "evidence",
  "memory",
  "cpu",
  "address",
  "port",
  "protocol",
]);
const filterLabels: Record<keyof ColumnFilters, string> = {
  name: "Process",
  pid: "PID",
  path: "Path",
  access: "Access",
  cpu_min: "CPU min",
  cpu_max: "CPU max",
  memory_min: "Memory min",
  memory_max: "Memory max",
  evidence: "Evidence",
};
const autoReloadOptions = [0, 5, 10, 15, 30, 60] as const;
function readHiddenColumns(): Set<ColumnKey> {
  try {
    const saved: unknown = JSON.parse(
      localStorage.getItem("oflh-hidden-columns") ?? "[]",
    );
    return new Set(
      Array.isArray(saved)
        ? saved.filter(
            (value: unknown): value is ColumnKey =>
              typeof value === "string" && columnKeys.has(value as ColumnKey),
          )
        : [],
    );
  } catch {
    return new Set();
  }
}
function formatFailureDetails(failure: unknown): string {
  if (failure instanceof Error) return failure.stack || failure.message;
  if (typeof failure === "object" && failure !== null) {
    const value = failure as { details?: unknown; stack?: unknown };
    if (typeof value.details === "string") return value.details;
    if (typeof value.stack === "string") return value.stack;
    try {
      return JSON.stringify(failure, null, 2);
    } catch {
      return String(failure);
    }
  }
  return String(failure);
}
export function App() {
  const modifier = /Mac|iPhone|iPad/.test(navigator.platform) ? "⌘" : "Ctrl";
  const shortcut = (keys: string) => (
    <kbd className="shortcut" aria-hidden="true">
      {modifier}
      {modifier === "Ctrl" ? "+" : ""}
      {keys}
    </kbd>
  );
  const [fontSize, setFontSize] = useState(() => {
    try {
      const saved = Number(localStorage.getItem("oflh-font-size"));
      return saved >= 12 && saved <= 18 ? saved : 14;
    } catch {
      return 14;
    }
  });
  const [autoReloadSeconds, setAutoReloadSeconds] = useState(0);
  useEffect(() => {
    document.documentElement.style.fontSize = `${fontSize}px`;
    try {
      localStorage.setItem("oflh-font-size", String(fontSize));
    } catch {
      /* Session setting remains usable. */
    }
  }, [fontSize]);
  useEffect(() => {
    try {
      localStorage.removeItem("oflh-auto-reload-seconds");
    } catch {
      // Automatic refresh remains session-only.
    }
  }, []);
  const [status, setStatus] = useState(initialStatus);
  const [view, setView] = useState<View>("processes");
  const [path, setPath] = useState("");
  const [pathEdited, setPathEdited] = useState(false);
  const previousTarget = useRef("");
  const [fileQuery, setFileQuery] = useState("");
  const [portQuery, setPortQuery] = useState("");
  const [portsPathOnly, setPortsPathOnly] = useState(false);
  const query = view === "ports" ? portQuery : fileQuery;
  const setQuery = view === "ports" ? setPortQuery : setFileQuery;
  const [sort, setSort] = useState<Sort>("relevance");
  const [descending, setDescending] = useState(false);
  const [fileColumns, setFileColumns] = useState<ColumnFilters>({});
  const [portColumns, setPortColumns] = useState<ColumnFilters>({});
  const [hiddenColumns, setHiddenColumns] = useState(readHiddenColumns);
  const tableColumns =
    view === "ports" ? portColumnDefinitions : fileColumnDefinitions;
  const toggleHiddenColumn = (column: ColumnKey) => {
    setHiddenColumns((current) => {
      const next = new Set(current);
      if (next.has(column)) next.delete(column);
      else next.add(column);
      try {
        localStorage.setItem("oflh-hidden-columns", JSON.stringify([...next]));
      } catch {
        // The current session can still use the changed column set.
      }
      return next;
    });
  };
  const [showColumns, setShowColumns] = useState(false);
  const columns = view === "ports" ? portColumns : fileColumns;
  const setColumns = view === "ports" ? setPortColumns : setFileColumns;
  const activeColumnFilters = Object.entries(columns).flatMap(([key, value]) =>
    value !== undefined && value !== "" && value !== "any"
      ? [{ key, label: filterLabels[key as keyof ColumnFilters], value }]
      : [],
  );
  const columnCount = activeColumnFilters.length;
  useEffect(() => {
    if (columnCount > 0) setShowColumns(true);
  }, [columnCount]);
  const [locks, setLocks] = useState(false);
  const [scope, setScope] = useState<{ key: string; name: string } | null>(
    null,
  );
  const [selected, setSelected] = useState(new Set<string>());
  const [activeRow, setActiveRow] = useState<{
    row: Row;
    revision: number;
  } | null>(null);
  const [focused, setFocused] = useState<string | null>(null);
  const [details, setDetails] = useState<Details | null>(null);
  const [context, setContext] = useState<Row | null>(null);
  const [confirmation, setConfirmation] = useState<Confirmation | null>(null);
  const [showSupport, setShowSupport] = useState(false);
  const [results, setResults] = useState<ActionResult[] | null>(null);
  const [acting, setActing] = useState(false);
  const [dragging, setDragging] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [errorDetails, setErrorDetails] = useState<string | null>(null);
  const [showErrorDetails, setShowErrorDetails] = useState(false);
  const [errorContext, setErrorContext] = useState(t("Desktop operation"));
  const [toast, setToast] = useState("");
  const [total, setTotal] = useState(0);
  const [recent, setRecent] = useState<{ id: number; display: string }[]>([]);
  const [recentQuery, setRecentQuery] = useState("");
  const filteredRecent = recent.filter((target) =>
    target.display
      .toLocaleLowerCase()
      .includes(recentQuery.toLocaleLowerCase()),
  );
  const [initialTheme] = useState(readTheme);
  const [theme, setTheme] = useState(initialTheme.theme);
  const [showThemeWelcome, setShowThemeWelcome] = useState(
    initialTheme.firstUse,
  );
  const searchRef = useRef<HTMLInputElement>(null);
  const apply = useCallback(
    (incoming: Status) =>
      setStatus((current) => acceptStatus(current, incoming)),
    [],
  );
  const report = useCallback(
    (failure: unknown, context: string = t("Desktop operation")) => {
      const message = errorMessage(failure);
      setError(
        context === t("Desktop operation") ? message : `${context}: ${message}`,
      );
      setErrorDetails(formatFailureDetails(failure));
      setShowErrorDetails(false);
      setErrorContext(context);
    },
    [],
  );
  useEffect(() => {
    if (!autoReloadSeconds || !status.revision) return;
    const interval = window.setInterval(() => {
      if (
        !status.scanning &&
        !confirmation &&
        !acting &&
        !pathEdited &&
        !showErrorDetails &&
        !context
      )
        void api.refresh().then(apply).catch(report);
    }, autoReloadSeconds * 1000);
    return () => window.clearInterval(interval);
  }, [
    acting,
    apply,
    autoReloadSeconds,
    confirmation,
    context,
    pathEdited,
    report,
    showErrorDetails,
    status.revision,
    status.scanning,
  ]);
  const tableQuery: TableQuery = {
    columns,
    text: query,
    sort,
    descending,
    handles: view === "handles",
    locks_only: view !== "ports" && locks,
    ports: view === "ports",
    ports_path_only: !!status.target && portsPathOnly,
    offset: 0,
    limit: 200,
    process_key: scope?.key,
  };
  useEffect(() => {
    let active = true;
    const subscriptions = [
      listen<Status>("scan-status", (event) => {
        if (active) apply(event.payload);
      }),
      listen("target-dropped", () => {
        if (active) {
          setView("processes");
          setScope(null);
        }
      }),
      listen<boolean>("drag-active", (event) => {
        if (active) setDragging(event.payload);
      }),
      listen<Failure>("desktop-error", (event) => {
        if (active) report(event.payload);
      }),
    ];
    Promise.all(subscriptions)
      .then(() => api.status())
      .then((value) => {
        if (active) apply(value);
      })
      .catch((failure) => {
        if (active) report(failure);
      });
    return () => {
      active = false;
      subscriptions.forEach((subscription) => {
        void subscription.then((unlisten) => unlisten()).catch(() => {});
      });
    };
  }, [apply, report]);
  useEffect(() => {
    if (status.target) {
      setPath(status.target);
      setPathEdited(false);
    }
    if (status.target !== previousTarget.current) {
      setSelected(new Set());
      setFocused(null);
      setScope(null);
      previousTarget.current = status.target;
    }
  }, [status.target]);
  useEffect(() => {
    if (status.error) report(status.error);
  }, [status.error, report]);
  useEffect(() => {
    let active = true;
    api
      .recent()
      .then((value) => {
        if (active) setRecent(value);
      })
      .catch(report);
    return () => {
      active = false;
    };
  }, [status.revision, report]);
  useEffect(() => {
    let active = true;
    setDetails((current) =>
      current?.process.process_key === focused ? current : null,
    );
    if (focused)
      api
        .details(status.revision, focused)
        .then((value) => {
          if (active)
            setDetails((current) =>
              current?.process.process_key === value.process.process_key
                ? { ...value, ancestors: current.ancestors }
                : value,
            );
        })
        .catch((failure) => {
          if (active) {
            setFocused(null);
            report(failure);
          }
        });
    return () => {
      active = false;
    };
  }, [focused, status.revision, report]);
  useEffect(() => {
    const media = matchMedia("(prefers-color-scheme: dark)");
    const update = () =>
      (document.documentElement.dataset.theme =
        theme === "system" ? (media.matches ? "vscode" : "light") : theme);
    update();
    media.addEventListener("change", update);
    try {
      if (!showThemeWelcome) localStorage.setItem("oflh-theme", theme);
    } catch {
      /* Theme still applies for this session. */
    }
    return () => media.removeEventListener("change", update);
  }, [theme, showThemeWelcome]);
  useEffect(() => {
    if (!toast) return;
    const timer = setTimeout(() => setToast(""), 2300);
    return () => clearTimeout(timer);
  }, [toast]);
  const runScan = (
    work: Promise<Status | null>,
    nextView: View = "processes",
    resetScope = true,
  ) => {
    setError(null);
    void work
      .then((value) => {
        if (value) {
          apply(value);
          setView(nextView);
          if (resetScope) setScope(null);
        }
      })
      .catch(report);
  };
  const refresh = () => runScan(api.refresh(), view, false);
  const openIssueReport = () => {
    const body = [
      t("## What happened?"),
      `${t("OFLH reported:")} ${error ?? t("an operation could not complete")}`,
      "",
      t("## Steps to reproduce"),
      t("1. Open OFLH Desktop and inspect a file or folder."),
      t("2. Right-click a process row (or open its details)."),
      `${t("3. Choose")} ${errorContext}.`,
      t("4. Note the result and any OS or file-manager dialog."),
      "",
      t("## Diagnostics"),
      `${t("OFLH version:")} ${status.version || t("unknown")}`,
      `${t("Platform:")} ${navigator.platform || t("unknown")}`,
      "```text",
      errorDetails || error || t("No diagnostic details were provided."),
      "```",
      "",
      t(
        "Please review this draft and remove any private paths or process details before submitting.",
      ),
    ].join("\n");
    void api
      .openIssue(t("Desktop operation could not complete"), body)
      .catch(report);
  };
  const copyErrorDetails = () => {
    const text = errorDetails || error || t("No diagnostic details available.");
    void api
      .copyDiagnostic(text)
      .then(() => setToast(t("Error details copied")))
      .catch(report);
  };
  const removeRecent = (id: number) => {
    void api
      .removeRecent(id)
      .then(() =>
        setRecent((current) => current.filter((item) => item.id !== id)),
      )
      .catch(report);
  };
  const clearRecent = () => {
    void api
      .clearRecent()
      .then(() => setRecent([]))
      .catch(report);
  };
  const copy = (field = "rows", reference?: string, keys = [...selected]) => {
    void api
      .copy(status.revision, keys, field, reference)
      .then(() =>
        setToast(
          field === "path"
            ? t("Path copied")
            : field === "filename"
              ? t("Filename copied")
              : t("Selection copied"),
        ),
      )
      .catch(report);
  };
  const reveal = (reference: string, containing = false) => {
    const operation = containing
      ? t("Open containing folder")
      : t("Reveal in file manager");
    void api
      .reveal(status.revision, reference, containing)
      .catch((failure) => report(failure, operation));
  };
  const selectAll = () => {
    void api
      .keys(status.revision, tableQuery)
      .then((keys) => setSelected(new Set(keys)))
      .catch(report);
  };
  const prepare = (force: boolean, keys = [...selected]) => {
    setContext(null);
    setActing(true);
    void api
      .prepare(status.revision, keys, force)
      .then((value) => {
        setActing(false);
        setConfirmation(value);
      })
      .catch(report)
      .finally(() => setActing(false));
  };
  const dismiss = () => {
    if (acting) return;
    setConfirmation(null);
    void api.dismiss().catch(report);
  };
  const chooseSupport = (destination: "coffee" | "sponsors") => {
    setShowSupport(false);
    void (destination === "coffee" ? api.donate() : api.openSponsors()).catch(
      report,
    );
  };
  const terminate = () => {
    if (!confirmation || acting) return;
    setActing(true);
    setError(null);
    setFocused(null);
    setDetails(null);
    void api
      .terminate(confirmation.ticket)
      .then((value) => {
        setConfirmation(null);
        setResults(value);
        setSelected(new Set());
        runScan(api.refresh(), view, false);
      })
      .catch(report)
      .finally(() => setActing(false));
  };
  useEffect(() => {
    const handle = (event: KeyboardEvent) => {
      if (document.querySelector("dialog[open]")) return;
      const editing =
        event.target instanceof HTMLInputElement ||
        event.target instanceof HTMLTextAreaElement ||
        (event.target instanceof HTMLElement && event.target.isContentEditable);
      const command = event.metaKey || event.ctrlKey;
      if (command && event.key === ",") {
        event.preventDefault();
        changeView("settings");
      } else if (
        !editing &&
        command &&
        !event.shiftKey &&
        /^[1-4]$/.test(event.key)
      ) {
        event.preventDefault();
        changeView(
          (["processes", "handles", "ports", "history"] as const)[
            Number(event.key) - 1
          ],
        );
      } else if (!editing && event.ctrlKey && event.key === "Tab") {
        event.preventDefault();
        const views = ["processes", "handles", "ports", "history"] as const;
        const index = views.findIndex((item) => item === view);
        changeView(views[(index + (event.shiftKey ? 3 : 1) + 4) % 4]);
      } else if (
        !editing &&
        command &&
        event.shiftKey &&
        event.key.toLowerCase() === "d"
      ) {
        event.preventDefault();
        if (focused) setFocused(null);
        else if (selected.size) setFocused([...selected][0]);
      } else if (
        (command && event.key.toLowerCase() === "f") ||
        (!editing && !command && event.key === "/")
      ) {
        event.preventDefault();
        if (!inspecting) changeView("processes");
        requestAnimationFrame(() => {
          searchRef.current?.focus();
          searchRef.current?.select();
        });
      } else if (editing && event.key === "Escape") {
        (event.target as HTMLElement).blur();
      } else if (
        (command && event.key.toLowerCase() === "r") ||
        event.key === "F5"
      ) {
        event.preventDefault();
        if (status.revision) refresh();
      } else if (command && event.key.toLowerCase() === "o") {
        event.preventDefault();
        runScan(api.choose(event.shiftKey));
      } else if (
        !editing &&
        command &&
        event.key.toLowerCase() === "a" &&
        (view === "processes" || view === "handles" || view === "ports")
      ) {
        event.preventDefault();
        selectAll();
      } else if (
        !editing &&
        command &&
        event.key.toLowerCase() === "c" &&
        selected.size
      ) {
        event.preventDefault();
        copy();
      } else if (!editing && event.key === "Escape") {
        setSelected(new Set());
        setFocused(null);
      }
    };
    window.addEventListener("keydown", handle);
    return () => window.removeEventListener("keydown", handle);
  });
  const changeSort = (next: Sort) => {
    if (next === sort) setDescending(!descending);
    else {
      setSort(next);
      setDescending(false);
    }
  };
  const changeView = (next: View) => {
    setView(next);
    setScope(null);
    setSort("relevance");
    setDescending(false);
    if (next === "ports" && !status.revision && !status.scanning)
      runScan(api.ports(), "ports");
  };
  const inspecting =
    view === "processes" || view === "handles" || view === "ports";
  return (
    <div
      className="app-shell"
      onContextMenu={(event) => event.preventDefault()}
    >
      <header className="app-header">
        <div className="brand">
          <span className="brand-symbol">
            <FileSearch size={22} />
          </span>
          <strong>OFLH</strong>
          <span className="brand-divider" />
          <span>Desktop</span>
          <span className="rc-badge">RC</span>
        </div>
        <div className="header-actions">
          {status.revision > 0 && (
            <>
              <label className="auto-refresh-control">
                <Timer size={14} />
                <span>{t("Auto")}</span>
                <select
                  aria-label={t("Automatic refresh interval")}
                  value={autoReloadSeconds}
                  onChange={(event) =>
                    setAutoReloadSeconds(Number(event.target.value))
                  }
                >
                  {autoReloadOptions.map((seconds) => (
                    <option key={seconds} value={seconds}>
                      {seconds === 0 ? t("Off") : `${seconds}s`}
                    </option>
                  ))}
                </select>
              </label>
              <details className="auto-refresh-info">
                <summary
                  aria-label={t("Automatic refresh information")}
                  title={t("About automatic refresh")}
                >
                  <Info size={15} />
                </summary>
                <div role="note">
                  <strong>{t("Automatic refresh")}</strong>
                  <p>
                    {t(
                      "Off by default. Choose an interval to repeat the current scan while OFLH is open.",
                    )}
                  </p>
                  <p>
                    {t(
                      "It can help with changing processes or ports. Results may change while you inspect, so manual refresh is often better for a focused check.",
                    )}
                  </p>
                </div>
              </details>
            </>
          )}
          <button
            disabled={!status.revision}
            onClick={refresh}
            title={`${t("Refresh")} (${modifier}+R ${t("or")} F5)`}
          >
            <RefreshCw size={14} className={status.scanning ? "spin" : ""} />
            {t("Refresh")}
            <kbd className="shortcut" aria-hidden="true">
              F5
            </kbd>
          </button>
        </div>
      </header>
      <div className="app-body">
        <nav className="sidebar" aria-label={t("Workspace")}>
          <div className="nav-section">{t("WORKSPACE")}</div>
          <button
            title={`${t("Processes")} (${modifier}+1)`}
            className={view === "processes" ? "active" : ""}
            onClick={() => changeView("processes")}
          >
            <Activity size={17} />
            {t("Processes")}
            <span>{status.processes || ""}</span>
            {shortcut("1")}
          </button>
          <button
            title={`${t("File usages")} (${modifier}+2)`}
            className={view === "handles" ? "active" : ""}
            onClick={() => changeView("handles")}
          >
            <Files size={17} />
            {t("File usages")}
            <span>{status.usages || ""}</span>
            {shortcut("2")}
          </button>
          <button
            title={`${t("Ports")} (${modifier}+3)`}
            className={view === "ports" ? "active" : ""}
            onClick={() => changeView("ports")}
          >
            <Network size={17} />
            {t("Ports")}
            <span>{status.ports || ""}</span>
            {shortcut("3")}
          </button>
          <button
            title={`${t("Recent targets")} (${modifier}+4)`}
            className={view === "history" ? "active" : ""}
            onClick={() => changeView("history")}
          >
            <History size={17} />
            {t("Recent targets")}
            {shortcut("4")}
          </button>
          <div className="sidebar-rule" />
          <div className="nav-section">{t("INSPECT TARGET")}</div>
          <button onClick={() => runScan(api.choose(false))}>
            <File size={16} />
            {t("Open file")}
          </button>
          <button onClick={() => runScan(api.choose(true))}>
            <FolderOpen size={17} />
            {t("Open folder")}
          </button>
          <div className="sidebar-bottom">
            <p>
              {t("Know what’s using")}
              <br />
              {t("your files.")}
            </p>
            <button
              className={view === "settings" ? "active" : ""}
              onClick={() => changeView("settings")}
            >
              <Settings size={17} />
              {t("Settings")}
            </button>
            <button
              className="github-link"
              onClick={() => void api.openProject().catch(report)}
            >
              <Star size={14} /> {t("Star on GitHub")}{" "}
              <ExternalLink size={12} />
            </button>
            <span className="version">
              {status.version === "development"
                ? t("Development")
                : status.version
                  ? `v${status.version}`
                  : "OFLH Desktop"}
            </span>
          </div>
        </nav>
        <main>
          {error && (
            <div className="error-banner" role="alert">
              <ShieldAlert size={17} />
              <div className="error-content">
                <div className="error-summary">
                  <strong>{t("Operation could not complete")}</strong>
                  <p>{error}</p>
                </div>
                <div className="error-actions">
                  <button
                    onClick={() => setShowErrorDetails((visible) => !visible)}
                    aria-expanded={showErrorDetails}
                  >
                    {showErrorDetails ? t("Hide details") : t("Details")}
                  </button>
                  <button onClick={openIssueReport}>
                    <ExternalLink size={13} /> {t("Open issue")}
                  </button>
                  <span>
                    {t(
                      "Review the draft and remove private paths before submitting.",
                    )}
                  </span>
                </div>
              </div>
              <button
                className="icon-button"
                aria-label={t("Dismiss error")}
                onClick={() => {
                  setError(null);
                  setErrorDetails(null);
                  setShowErrorDetails(false);
                }}
              >
                <X size={16} />
              </button>
            </div>
          )}
          {inspecting ? (
            <>
              <div className="workspace-heading">
                <div>
                  <div className="eyebrow">
                    {view === "ports"
                      ? t("NETWORK INSPECTION")
                      : t("FILE INSPECTION")}
                  </div>
                  <h1>
                    {view === "ports"
                      ? t("Local ports")
                      : view === "handles"
                        ? t("File usages")
                        : t("Processes")}
                  </h1>
                  <p>
                    {view === "ports"
                      ? t(
                          "Find local TCP listeners, bound UDP sockets and their captured owners.",
                        )
                      : status.target
                        ? t(
                            "Processes referencing your target and its contents.",
                          )
                        : t(
                            "Find out which processes are using a file or folder.",
                          )}
                  </p>
                </div>
                <button
                  className="primary"
                  onClick={() => runScan(api.choose(true))}
                >
                  <FolderOpen size={15} />
                  {t("Open folder")}
                </button>
              </div>
              {view !== "ports" && (
                <form
                  className="target-bar"
                  onSubmit={(event) => {
                    event.preventDefault();
                    runScan(
                      !pathEdited && status.target
                        ? api.refresh()
                        : api.inspect(path),
                    );
                  }}
                >
                  <FolderOpen size={17} />
                  <input
                    aria-label={t("Target file or folder path")}
                    placeholder={t("Paste a file or folder path…")}
                    value={path}
                    onChange={(event) => {
                      setPath(event.target.value);
                      setPathEdited(true);
                    }}
                    spellCheck={false}
                  />
                  <button type="submit" disabled={!path.trim()}>
                    {t("Inspect")}
                    <ArrowRight size={14} />
                  </button>
                </form>
              )}
              {view !== "ports" && !status.target && !status.scanning ? (
                <div className="welcome">
                  <div className="welcome-icon">
                    <FileSearch size={36} />
                  </div>
                  <h2>{t("A clear view of files in use.")}</h2>
                  <p>
                    {t("Drop a file or folder anywhere in this window.")}
                    <br />
                    {t("OFLH will find the processes referencing it.")}
                  </p>
                  <div className="welcome-actions">
                    <button
                      className="primary"
                      onClick={() => runScan(api.choose(false))}
                    >
                      <File size={15} />
                      {t("Choose file")}
                    </button>
                    <button onClick={() => runScan(api.choose(true))}>
                      <FolderOpen size={15} />
                      {t("Choose folder")}
                    </button>
                  </div>
                  <div className="welcome-note">
                    <ShieldAlert size={15} />
                    <span>
                      {t("Open files do not necessarily mean locked files.")}
                      <br />
                      {t(
                        "OFLH shows lock evidence when the operating system provides it.",
                      )}
                    </span>
                  </div>
                </div>
              ) : (
                <>
                  <div className="search-toolbar">
                    <div className="search-box">
                      <Search size={15} />
                      <input
                        ref={searchRef}
                        aria-label={t("Search loaded results")}
                        title={t(
                          "Search ({shortcut}+F or /); Escape returns to the workspace",
                        ).replace("{shortcut}", modifier)}
                        placeholder={
                          view === "ports"
                            ? t("Search ports… e.g. 80, port:8080, tcp")
                            : t("Search names, PIDs, paths…")
                        }
                        value={query}
                        onChange={(event) => setQuery(event.target.value)}
                      />
                      {!query && shortcut("F")}
                      {query && (
                        <button
                          className="icon-button"
                          aria-label={t("Clear search")}
                          onClick={() => setQuery("")}
                        >
                          <X size={13} />
                        </button>
                      )}
                    </div>
                    {view === "ports" ? (
                      <label className="checkbox-label">
                        <input
                          type="checkbox"
                          disabled={!status.target}
                          checked={!!status.target && portsPathOnly}
                          onChange={(event) =>
                            setPortsPathOnly(event.target.checked)
                          }
                        />
                        {t("Target processes only")}
                      </label>
                    ) : (
                      <label className="checkbox-label">
                        <input
                          type="checkbox"
                          checked={locks}
                          onChange={(event) => setLocks(event.target.checked)}
                        />
                        {t("Lock evidence only")}
                      </label>
                    )}

                    <button className="select-all" onClick={selectAll}>
                      {t("Select all")}
                    </button>
                    <details className="column-picker">
                      <summary>
                        <Columns3 size={14} />
                        {t("Columns")}
                        <ChevronDown
                          size={12}
                          className="column-picker-caret"
                        />
                      </summary>
                      <div
                        className="column-picker-menu"
                        role="group"
                        aria-label={t("Visible columns")}
                      >
                        <span className="column-picker-heading">
                          {t("SHOW IN GRID")}
                        </span>
                        {tableColumns.map((column) => (
                          <label key={column.key}>
                            <input
                              type="checkbox"
                              checked={!hiddenColumns.has(column.key)}
                              disabled={column.key === "process"}
                              onChange={() => toggleHiddenColumn(column.key)}
                            />
                            {t(column.label)}
                          </label>
                        ))}
                      </div>
                    </details>
                    <button
                      aria-expanded={showColumns}
                      disabled={columnCount > 0}
                      title={
                        columnCount > 0
                          ? t("Clear applied filters before closing")
                          : t("Open column filters")
                      }
                      onClick={() => setShowColumns((visible) => !visible)}
                    >
                      <SlidersHorizontal size={14} />
                      {t("Column filters")}
                      {columnCount ? ` (${columnCount})` : ""}
                    </button>
                    <span className="muted result-count">
                      {total} {t("results")}
                    </span>
                  </div>
                  {showColumns && (
                    <ColumnFilterPanel
                      key={view === "ports" ? "port-filters" : "file-filters"}
                      value={columns}
                      ports={view === "ports"}
                      apply={setColumns}
                      close={() => setShowColumns(false)}
                      canClose={columnCount === 0}
                    />
                  )}
                  {columnCount > 0 && (
                    <div
                      className="active-filter-banner"
                      role="status"
                      aria-label={t("Applied column filters")}
                    >
                      <strong>
                        <SlidersHorizontal size={14} />
                        {t("Filters active")}
                      </strong>
                      <div className="active-filter-values">
                        {activeColumnFilters.map((filter) => (
                          <span key={filter.key}>
                            {t(filter.label)}: {String(filter.value)}
                          </span>
                        ))}
                      </div>
                      <button onClick={() => setColumns({})}>
                        <X size={13} />
                        {t("Clear filters")}
                      </button>
                    </div>
                  )}
                  {query && view !== "ports" && (
                    <div className="search-explanation">
                      {t(
                        "Search includes full paths. A shared folder name can match every row; use the Process name column filter to narrow by name.",
                      )}
                    </div>
                  )}
                  {scope && (
                    <div className="scope-bar">
                      {t("Showing")}{" "}
                      {view === "ports" ? t("local ports") : t("file usages")}{" "}
                      for <strong>{scope.name}</strong>
                      <button onClick={() => setScope(null)}>
                        <X size={12} />
                        {t("Clear process filter")}
                      </button>
                    </div>
                  )}
                  {selected.size > 0 ? (
                    <div className="selection-toolbar">
                      <strong>
                        {selected.size}{" "}
                        {selected.size === 1 ? t("process") : t("processes")}{" "}
                        {t("selected")}
                      </strong>
                      <span className="muted">
                        {t("May include processes outside this view")}
                      </span>
                      <button onClick={() => copy()}>
                        <Copy size={13} />
                        {t("Copy")}
                      </button>
                      <button disabled={acting} onClick={() => prepare(false)}>
                        {t("Terminate…")}
                      </button>
                      <button
                        className="danger-text"
                        disabled={acting}
                        onClick={() => prepare(true)}
                      >
                        {t("Force terminate…")}
                      </button>
                      <button
                        className="icon-button"
                        aria-label={t("Clear selection")}
                        onClick={() => setSelected(new Set())}
                      >
                        <X size={14} />
                      </button>
                    </div>
                  ) : (
                    <div className="selection-toolbar selection-hint">
                      {t(
                        "Click a row to inspect · Use the panel button to close details",
                      )}
                    </div>
                  )}
                  <div className="results-workspace">
                    <Table
                      fontSize={fontSize}
                      target={status.target}
                      expandedKey={
                        focused &&
                        activeRow?.revision === status.revision &&
                        activeRow.row.process_key === focused
                          ? activeRow.row.key
                          : null
                      }
                      onToggle={(row) => {
                        const closing =
                          focused === row.process_key &&
                          activeRow?.row.key === row.key;
                        setActiveRow({ row, revision: status.revision });
                        setFocused(closing ? null : row.process_key);
                      }}
                      key={view === "ports" ? "ports" : "files"}
                      revision={status.revision}
                      query={tableQuery}
                      hiddenColumns={hiddenColumns}
                      selected={selected}
                      focused={focused}
                      onSelect={(row, additive) => {
                        setSelected((current) =>
                          selectKey(current, row.process_key, additive),
                        );
                        setActiveRow({ row, revision: status.revision });
                        setFocused(row.process_key);
                      }}
                      onOpen={(row) => {
                        setActiveRow({ row, revision: status.revision });
                        setFocused(row.process_key);
                      }}
                      onContext={(row) => {
                        if (status.scanning) {
                          setToast(
                            t(
                              "Wait for the current scan to finish before opening row actions.",
                            ),
                          );
                          return;
                        }
                        setContext(row);
                        setActiveRow({ row, revision: status.revision });
                        setFocused(row.process_key);
                      }}
                      onSort={changeSort}
                      onTotal={setTotal}
                      onError={report}
                    />
                    {details && (
                      <Inspector
                        key={details.process.process_key}
                        terminateCurrent={(force) =>
                          prepare(force, [details.process.process_key])
                        }
                        terminateAncestor={(key, force) => {
                          setActing(true);
                          void api
                            .prepareAncestor(
                              details.process.process_key,
                              key,
                              force,
                            )
                            .then((value) => {
                              setActing(false);
                              setConfirmation(value);
                            })
                            .catch(report)
                            .finally(() => setActing(false));
                        }}
                        details={details}
                        row={
                          activeRow?.revision === status.revision &&
                          activeRow.row.process_key ===
                            details.process.process_key
                            ? activeRow.row
                            : null
                        }
                        close={() => setFocused(null)}
                        copy={(field, reference) =>
                          copy(field, reference, [details.process.process_key])
                        }
                        reveal={reveal}
                        ports={() => {
                          setView("ports");
                          setScope({
                            key: details.process.process_key,
                            name: details.process.name,
                          });
                          setPortQuery("");
                        }}
                        inspectFolder={() =>
                          runScan(
                            api.followProcess(
                              status.revision,
                              details.process.process_key,
                            ),
                          )
                        }
                        handles={() => {
                          setView("handles");
                          setScope({
                            key: details.process.process_key,
                            name: details.process.name,
                          });
                          setFileQuery("");
                        }}
                      />
                    )}
                  </div>
                  <div className="evidence-note">
                    <ShieldAlert size={13} />
                    <span>
                      {view === "ports"
                        ? "Local bindings do not prove external reachability. Unknown owners cannot be terminated."
                        : "File usage is not proof of a lock. Windows resource users are not proven lock owners."}
                    </span>
                    {status.warnings.length > 0 && (
                      <details>
                        <summary>
                          {status.warnings.length} coverage notices
                        </summary>
                        <ul>
                          {status.warnings.map((warning, index) => (
                            <li key={index}>{t(warning)}</li>
                          ))}
                        </ul>
                      </details>
                    )}
                  </div>
                </>
              )}
            </>
          ) : view === "history" ? (
            <div className="content-page">
              <span className="eyebrow">{t("SAVED HISTORY")}</span>
              <h1>{t("Recent targets")}</h1>
              <p className="muted">
                {t(
                  "Reinspect targets saved on this device, even after reopening OFLH.",
                )}
              </p>
              {recent.length ? (
                <>
                  <div className="recent-toolbar">
                    <label className="recent-search">
                      <Search size={15} />
                      <input
                        type="search"
                        aria-label={t("Search recent targets")}
                        placeholder={t("Filter recent targets")}
                        value={recentQuery}
                        onChange={(event) => setRecentQuery(event.target.value)}
                      />
                    </label>
                    <span className="muted recent-count">
                      {filteredRecent.length} {t("of")} {recent.length}
                    </span>
                    <button
                      className="danger-text"
                      disabled={!recent.length}
                      onClick={clearRecent}
                    >
                      <Trash2 size={14} />
                      {t("Clear all")}
                    </button>
                  </div>
                  {filteredRecent.length ? (
                    <div
                      className="recent-list"
                      aria-label={t("Recent targets")}
                    >
                      {filteredRecent.map((target) => (
                        <div className="recent-entry" key={target.id}>
                          <button
                            className="recent-target"
                            title={target.display}
                            onClick={() => runScan(api.revisit(target.id))}
                          >
                            <FolderOpen size={18} />
                            <span className="mono">{target.display}</span>
                            <ArrowRight size={15} />
                          </button>
                          <button
                            className="icon-button recent-remove"
                            aria-label={`${t("Remove")} ${target.display}`}
                            title={t("Remove from recent targets")}
                            onClick={() => removeRecent(target.id)}
                          >
                            <Trash2 size={15} />
                          </button>
                        </div>
                      ))}
                    </div>
                  ) : (
                    <div className="empty recent-empty">
                      <Search size={26} />
                      <h3>{t("No matching recent targets")}</h3>
                      <button onClick={() => setRecentQuery("")}>
                        {t("Clear search")}
                      </button>
                    </div>
                  )}
                </>
              ) : (
                <div className="empty">
                  <History size={30} />
                  <h3>{t("No recent targets")}</h3>
                  <p>{t("Choose a file or folder to start an inspection.")}</p>
                </div>
              )}
            </div>
          ) : (
            <div className="content-page">
              <span className="eyebrow">{t("PREFERENCES")}</span>
              <h1>{t("Settings")}</h1>
              <section className="setting-section">
                <div>
                  <h3>{t("Appearance")}</h3>
                  <p className="muted">
                    {t("Choose a theme or follow your system.")}
                  </p>
                </div>
                <ThemePicker theme={theme} onChange={setTheme} />
              </section>
              <section className="setting-section">
                <div>
                  <h3>{t("Interface font size")}</h3>
                  <p className="muted">
                    {t(
                      "Scale text throughout the workspace, including tables and process details.",
                    )}
                  </p>
                </div>
                <div className="font-setting">
                  <label htmlFor="font-size">{t("Size")}</label>
                  <select
                    id="font-size"
                    value={fontSize}
                    onChange={(event) =>
                      setFontSize(Number(event.target.value))
                    }
                  >
                    {[12, 13, 14, 15, 16, 17, 18].map((size) => (
                      <option key={size} value={size}>
                        {size} px
                      </option>
                    ))}
                  </select>
                  <button onClick={() => setFontSize(14)}>
                    {t("Reset to default")}
                  </button>
                </div>
              </section>
              <section className="setting-section">
                <div>
                  <h3>{t("About OFLH")}</h3>
                  <p>
                    {t("OFLH is an independent project by")}{" "}
                    <button
                      className="inline-link"
                      onClick={() => void api.openProfile().catch(report)}
                    >
                      Karim Zouine
                    </button>
                    {t(", built in spare time. There is no company behind it.")}
                  </p>
                  <p className="muted">
                    {t(
                      "If OFLH helps your work, you can support its ongoing development.",
                    )}
                  </p>
                </div>
                <div className="inline-actions about-links">
                  <button onClick={() => void api.openProject().catch(report)}>
                    <ExternalLink size={14} /> {t("View project on GitHub")}
                  </button>
                  {status.pull_request_url && (
                    <button
                      onClick={() => void api.openPullRequest().catch(report)}
                    >
                      <ExternalLink size={14} /> {t("View pull request")}
                    </button>
                  )}
                  {status.build_url && (
                    <button onClick={() => void api.openBuild().catch(report)}>
                      <ExternalLink size={14} /> {t("View Actions run")}
                    </button>
                  )}
                </div>
                <p className="muted about-version">
                  OFLH Desktop {status.version} · {t("MIT license")}
                </p>
                {status.commit && (
                  <p className="muted about-version">
                    Commit <code>{status.commit.slice(0, 12)}</code>
                  </p>
                )}
              </section>
              <section className="setting-section shortcuts">
                <h3>
                  <Keyboard size={17} />
                  {t("Keyboard shortcuts")}
                </h3>
                <dl>
                  <dt>
                    {t("Processes / File usages / Ports / Recent targets")}
                  </dt>
                  <dd>{modifier}+1 / 2 / 3 / 4</dd>
                  <dt>{t("Next / previous workspace")}</dt>
                  <dd>Ctrl+Tab / Ctrl+Shift+Tab</dd>
                  <dt>{t("Toggle selected process details")}</dt>
                  <dd>{modifier}+Shift+D</dd>
                  <dt>{t("Settings")}</dt>
                  <dd>{modifier}+,</dd>
                  <dt>{t("Open file / folder")}</dt>
                  <dd>Ctrl / ⌘ O · {t("Shift for folder")}</dd>
                  <dt>{t("Search results")}</dt>
                  <dd>
                    {modifier}+F {t("or")} / · {t("Escape leaves search")}
                  </dd>
                  <dt>{t("Refresh target")}</dt>
                  <dd>Ctrl / ⌘ R or F5</dd>
                  <dt>{t("Select all matching processes")}</dt>
                  <dd>Ctrl / ⌘ A</dd>
                  <dt>{t("Copy selected processes")}</dt>
                  <dd>Ctrl / ⌘ C</dd>
                  <dt>{t("Navigate / toggle selection")}</dt>
                  <dd>↑ ↓ / Space</dd>
                  <dt>{t("Context actions")}</dt>
                  <dd>Shift F10</dd>
                  <dt>{t("Clear selection and details")}</dt>
                  <dd>Escape</dd>
                </dl>
              </section>
              {import.meta.env.DEV && (
                <section className="setting-section developer-tools">
                  <div>
                    <h3>{t("Developer options")}</h3>
                    <p className="muted">
                      {t(
                        "Preview the operation-error details and issue-report flow.",
                      )}
                    </p>
                  </div>
                  <button
                    onClick={() =>
                      report(
                        {
                          kind: "desktop_integration",
                          message: "Sample file-manager operation failed",
                          os_code: 2,
                          details:
                            "Sample reveal request\nCaused by: File manager is unavailable\nRust backtrace: development preview",
                        },
                        "Reveal in file manager",
                      )
                    }
                  >
                    <ShieldAlert size={14} /> {t("Show sample error")}
                  </button>
                </section>
              )}
              <section className="setting-section">
                <h3>{t("Search and inspection")}</h3>
                <p>
                  {t(
                    "Search runs over the loaded Rust snapshot and supports substrings, wildcards (*) and word-boundary abbreviations. In Ports, bare digits match port fragments; port:8080 matches exactly 8080. Combine port queries with tcp, udp, ipv4, ipv6 or pid:1234. Refresh performs a new system scan.",
                  )}
                </p>
                <p className="muted">
                  {t(
                    "Unknown CPU and memory stay unavailable. Access permissions may hide processes. Normal termination never escalates to force termination. Unsaved work can be lost when stopping a process.",
                  )}
                </p>
              </section>
            </div>
          )}
        </main>
      </div>
      <footer className="statusbar">
        <span className="status-current" role="status">
          {status.scanning && <LoaderCircle size={13} className="spin" />}
          {status.scanning
            ? t("Scanning")
            : status.revision
              ? t("Inspection complete")
              : t("Ready to inspect")}
          {status.scanning && (
            <button
              className="status-cancel"
              onClick={() => void api.cancel().then(apply).catch(report)}
            >
              {t("Cancel")}
            </button>
          )}
        </span>
        <span className="status-metrics">
          {status.processes} {t("file users")} · {status.usages}{" "}
          {t("file usages")} · {status.ports} {t("Ports").toLocaleLowerCase()}
        </span>
        <span className="footer-end">
          <span className="footer-attribution">
            {selected.size ? `${selected.size} ${t("selected")} · ` : ""}
            {t("Independent project by")}{" "}
            <button
              className="footer-profile"
              title={`${t("Open")} Karim Zouine ${t("GitHub profile")}`}
              onClick={() => void api.openProfile().catch(report)}
            >
              Karim Zouine
            </button>
            , {t("built in spare time")}
          </span>
          <button
            className="footer-link"
            title={t("Choose how to support OFLH")}
            onClick={() => setShowSupport(true)}
          >
            {t("Donate")}
          </button>
        </span>
      </footer>
      {dragging && (
        <div className="drop-overlay">
          <div>
            <FolderOpen size={42} />
            <h2>{t("Drop file or folder to inspect")}</h2>
            <p>{t("One target at a time")}</p>
          </div>
        </div>
      )}
      {toast && (
        <div className="toast" role="status">
          <Check size={15} />
          {toast}
        </div>
      )}
      {showErrorDetails && error && (
        <Modal
          title={t("Operation details")}
          close={() => setShowErrorDetails(false)}
        >
          <p className="muted">
            {t(
              "Diagnostic details can contain local paths or other private information. Review them before sharing.",
            )}
          </p>
          <pre className="error-details-modal">{errorDetails || error}</pre>
          <div className="modal-actions">
            <button onClick={copyErrorDetails}>
              <Copy size={14} /> {t("Copy details")}
            </button>
            <button onClick={() => setShowErrorDetails(false)}>
              {t("Close")}
            </button>
          </div>
        </Modal>
      )}
      {showSupport && (
        <Modal title={t("Support OFLH")} close={() => setShowSupport(false)}>
          <p className="support-intro">
            {t(
              "Support is optional. Choose the route that best fits how you would like to help this independent project.",
            )}
          </p>
          <div className="support-options">
            <section className="support-option">
              <h3>
                <Coffee size={17} /> {t("Buy Me a Coffee")}
              </h3>
              <p>
                <strong>{t("Good for:")}</strong>{" "}
                {t("a simple contribution from an individual.")}
              </p>
              <p className="muted">
                <strong>{t("Trade-off:")}</strong>{" "}
                {t("checkout is handled by a separate service.")}
              </p>
              <button
                className="primary"
                onClick={() => chooseSupport("coffee")}
              >
                {t("Continue with Buy Me a Coffee")}
                <ExternalLink size={13} />
              </button>
            </section>
            <section className="support-option">
              <h3>
                <Star size={17} /> {t("GitHub Sponsors")}
              </h3>
              <p>
                <strong>{t("Good for:")}</strong>{" "}
                {t("ongoing sponsorship and company support.")}
              </p>
              <p className="muted">
                <strong>{t("Trade-off:")}</strong>{" "}
                {t("sponsorship uses GitHub’s checkout flow.")}
              </p>
              <button
                className="primary"
                onClick={() => chooseSupport("sponsors")}
              >
                {t("Continue to GitHub Sponsors")}
                <ExternalLink size={13} />
              </button>
            </section>
          </div>
        </Modal>
      )}
      {context && (
        <Modal
          title={context.name || `PID ${context.pid}`}
          close={() => setContext(null)}
        >
          <p className="muted">
            {t("Actions for PID")} {context.pid}
          </p>
          <div className="context-actions">
            {context.port && (
              <>
                <button
                  onClick={() => {
                    copy("port", context.port!.reference);
                    setContext(null);
                  }}
                >
                  {t("Copy port")}
                </button>
                <button
                  onClick={() => {
                    copy("endpoint", context.port!.reference);
                    setContext(null);
                  }}
                >
                  {t("Copy local endpoint")}
                </button>
                <hr />
              </>
            )}
            <button
              onClick={() => {
                copy("path", context.path_ref);
                setContext(null);
              }}
              disabled={!context.path}
            >
              <Copy size={15} />
              {t("Copy path")}
            </button>
            <button
              onClick={() => {
                copy("filename", context.path_ref);
                setContext(null);
              }}
              disabled={!context.path}
            >
              {t("Copy filename")}
            </button>
            {context.pid > 0 && (
              <button
                onClick={() => {
                  copy("pid", undefined, [context.process_key]);
                  setContext(null);
                }}
              >
                {t("Copy PID")}
              </button>
            )}
            <button
              onClick={() => {
                copy("name", undefined, [context.process_key]);
                setContext(null);
              }}
            >
              {t("Copy process name")}
            </button>
            <button
              onClick={() => {
                reveal(context.path_ref);
                setContext(null);
              }}
              disabled={!context.path}
            >
              <ExternalLink size={15} />
              {t("Reveal in file manager")}
            </button>
            <button
              onClick={() => {
                reveal(context.path_ref, true);
                setContext(null);
              }}
              disabled={!context.path}
            >
              {t("Open containing folder")}
            </button>
            <hr />
            <button
              onClick={() => {
                setFocused(context.process_key);
                setContext(null);
              }}
            >
              {t("View process details")}
            </button>
            <button
              onClick={() => {
                setView("handles");
                setScope({ key: context.process_key, name: context.name });
                setQuery("");
                setContext(null);
              }}
            >
              {t("View matching handles")}
            </button>
            {details?.process.process_key === context.process_key &&
              details.can_inspect_folder && (
                <button
                  onClick={() => {
                    runScan(
                      api.followProcess(status.revision, context.process_key),
                    );
                    setContext(null);
                  }}
                >
                  {t("Inspect owner folder")}
                </button>
              )}
            {context.actionable && (
              <>
                <hr />
                <button onClick={() => prepare(false, [context.process_key])}>
                  {t("Terminate…")}
                </button>
                <button
                  className="danger-text"
                  onClick={() => prepare(true, [context.process_key])}
                >
                  {t("Force terminate…")}
                </button>
              </>
            )}
          </div>
        </Modal>
      )}
      {showThemeWelcome && (
        <Modal
          title={t("Make OFLH yours")}
          close={() => setShowThemeWelcome(false)}
        >
          <p>
            {t(
              "Choose your workspace theme. Preview it now; you can change it anytime in Settings.",
            )}
          </p>
          <ThemePicker theme={theme} onChange={setTheme} />
          <div className="modal-actions">
            <button
              className="primary"
              data-default-focus
              onClick={() => setShowThemeWelcome(false)}
            >
              {t("Start inspecting")}
            </button>
          </div>
        </Modal>
      )}
      {confirmation && (
        <Modal
          title={
            confirmation.force
              ? t("Force terminate processes?")
              : t("Terminate processes?")
          }
          danger={confirmation.force}
          close={dismiss}
        >
          <div className="confirmation-note">
            <ShieldAlert size={22} />
            <p>
              {confirmation.force
                ? t(
                    "Force termination stops these processes without allowing normal cleanup. Unsaved work may be lost.",
                  )
                : t(
                    "Request these processes to stop. Unsaved work may be lost. This will not escalate to force termination.",
                  )}
            </p>
          </div>
          <p>
            <strong>
              {confirmation.targets.length}{" "}
              {confirmation.targets.length === 1
                ? t("process")
                : t("processes")}
            </strong>{" "}
            —{" "}
            {t(
              "all targets are listed below, including any hidden by filters.",
            )}
          </p>
          <ul className="confirmation-list">
            {confirmation.targets.map((target) => (
              <li key={target.key}>
                <strong>{target.name || t("(unnamed)")}</strong>
                <code>PID {target.pid}</code>
              </li>
            ))}
          </ul>
          <p className="hint">
            {t(
              "OFLH validates each captured process identity again before sending the request.",
            )}
          </p>
          <div className="modal-actions">
            <button
              data-default-focus
              autoFocus
              disabled={acting}
              onClick={dismiss}
            >
              {t("Cancel")}
            </button>
            <button className="danger" disabled={acting} onClick={terminate}>
              {acting && <LoaderCircle size={14} className="spin" />}
              {acting
                ? t("Waiting for process exit…")
                : confirmation.force
                  ? t("Force terminate")
                  : t("Terminate")}
            </button>
          </div>
        </Modal>
      )}
      {results && (
        <Modal title={t("Termination results")} close={() => setResults(null)}>
          <p role="status">
            {status.scanning ? (
              <>
                <LoaderCircle size={14} className="spin" />{" "}
                {t("Updating results…")}
              </>
            ) : (
              t(
                "Results refreshed. Exit checks use the original process identity.",
              )
            )}
          </p>
          <ul className="action-results">
            {results.map((result) => (
              <li key={result.pid}>
                <code>PID {result.pid}</code>
                <span className={result.error ? "danger-text" : ""}>
                  {result.outcome === "exited" ? (
                    <>
                      <Check size={15} /> {t("Process exited")}
                    </>
                  ) : result.outcome === "still_running" ? (
                    t("Request sent · still running after 1.5 seconds")
                  ) : result.outcome === "unverified" ? (
                    t("Request sent · couldn’t verify exit")
                  ) : result.error?.kind === "identity_changed" ? (
                    t(
                      "Process changed or already exited. No termination was sent.",
                    )
                  ) : (
                    result.error?.message || t("Termination request failed")
                  )}
                  {result.outcome === "unverified" && result.error && (
                    <small>{result.error.message}</small>
                  )}
                </span>
              </li>
            ))}
          </ul>
          <div className="modal-actions">
            <button onClick={refresh} disabled={status.scanning}>
              <RefreshCw size={14} /> {t("Refresh again")}
            </button>
            <button data-default-focus onClick={() => setResults(null)}>
              {t("Done")}
            </button>
          </div>
        </Modal>
      )}
    </div>
  );
}

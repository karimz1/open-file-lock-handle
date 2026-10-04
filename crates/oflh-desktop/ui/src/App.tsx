import { forceRecoveryTargets } from "./termination";
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
  Maximize2,
  Minimize2,
  Network,
  PanelLeftClose,
  PanelLeftOpen,
  RefreshCw,
  Search,
  ShieldAlert,
  Timer,
  Trash2,
  X,
  ZoomIn,
  ZoomOut,
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
import { readTheme, ThemePicker, useAppliedTheme } from "./Themes";
import {
  Table,
  fileColumns as fileColumnDefinitions,
  portColumns as portColumnDefinitions,
  type ColumnKey,
} from "./Table";
import { ColumnFilterPanel } from "./ColumnFilters";
import { AboutDialog } from "./AboutDialog";
import { SettingsMenu } from "./SettingsMenu";
import { useUpdates } from "./useUpdates";
import { UpdateBanner } from "./UpdateBanner";
import { UpdateFeedback } from "./UpdateFeedback";
import {
  languageOptions,
  localePreference,
  t,
  tValue,
  type MessageKey,
} from "./i18n";

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
const filterLabels: Record<keyof ColumnFilters, MessageKey> = {
  name: "app.k_process",
  pid: "app.k_pid",
  path: "app.k_path",
  access: "inspector.k_access",
  cpu_min: "app.k_cpu_min",
  cpu_max: "app.k_cpu_max",
  memory_min: "inspector.k_memory_min",
  memory_max: "inspector.k_memory_max",
  evidence: "filters.k_evidence",
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
  const updates = useUpdates();
  const [showAbout, setShowAbout] = useState(false);

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
      return saved >= 12 && saved <= 24 ? saved : 14;
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
  // Font size already scales the whole interface (see the "Scalable controls"
  // rules in style.css), so zoom in/out just steps the same value.
  const zoomPercent = Math.round((fontSize / 14) * 100);
  const adjustZoom = (steps: number) =>
    setFontSize((current) => Math.min(24, Math.max(12, current + steps * 2)));
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
  const [maximized, setMaximized] = useState(false);
  const [sidebarCollapsed, setSidebarCollapsed] = useState(() => {
    try {
      return localStorage.getItem("oflh-sidebar-collapsed") === "1";
    } catch {
      return false;
    }
  });
  const sidebarIsCollapsed = sidebarCollapsed || maximized;
  const toggleSidebarCollapsed = () => {
    setSidebarCollapsed((current) => {
      const next = !current;
      try {
        localStorage.setItem("oflh-sidebar-collapsed", next ? "1" : "0");
      } catch {
        // The current session can still use the changed preference.
      }
      return next;
    });
  };
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
  const [ancestorOwner, setAncestorOwner] = useState<string | null>(null);
  const [resultContext, setResultContext] = useState<{
    confirmation: Confirmation;
    ancestorOwner: string | null;
  } | null>(null);
  const [showSupport, setShowSupport] = useState(false);
  const [results, setResults] = useState<ActionResult[] | null>(null);
  const [acting, setActing] = useState(false);
  const [dragging, setDragging] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [errorDetails, setErrorDetails] = useState<string | null>(null);
  const [showErrorDetails, setShowErrorDetails] = useState(false);
  const [errorContext, setErrorContext] = useState(
    t("app.k_desktop_operation"),
  );
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
  const appliedTheme = useAppliedTheme(theme);
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
    (failure: unknown, context: string = t("app.k_desktop_operation")) => {
      const message = errorMessage(failure);
      setError(
        context === t("app.k_desktop_operation")
          ? message
          : `${context}: ${message}`,
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
    document.documentElement.dataset.theme = appliedTheme;
    try {
      if (!showThemeWelcome) localStorage.setItem("oflh-theme", theme);
    } catch {
      /* Theme still applies for this session. */
    }
  }, [theme, appliedTheme, showThemeWelcome]);
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
      t("app.k_what_happened"),
      `${t("app.k_oflh_reported")} ${error ?? t("diagnostics.k_an_operation_could_not_complete")}`,
      "",
      t("diagnostics.k_steps_to_reproduce"),
      t("inspection.k_1_open_oflh_desktop_and_inspect_a_file_or_folder"),
      t("app.k_2_right_click_a_process_row_or_open_its_details"),
      `${t("app.k_3_choose")} ${errorContext}.`,
      t("diagnostics.k_4_note_the_result_and_any_os_or_file_ma_9ec676ec"),
      "",
      t("diagnostics.k_diagnostics"),
      `${t("diagnostics.k_oflh_version")} ${status.version || t("app.k_unknown")}`,
      `${t("diagnostics.k_platform")} ${navigator.platform || t("app.k_unknown")}`,
      "```text",
      errorDetails ||
        error ||
        t("diagnostics.k_no_diagnostic_details_were_provided"),
      "```",
      "",
      t("diagnostics.k_please_review_this_draft_and_remove_any_3d3c9702"),
    ].join("\n");
    void api
      .openIssue(t("diagnostics.k_desktop_operation_could_not_complete"), body)
      .catch(report);
  };
  const copyErrorDetails = () => {
    const text =
      errorDetails ||
      error ||
      t("diagnostics.k_no_diagnostic_details_available");
    void api
      .copyDiagnostic(text)
      .then(() => setToast(t("selection.k_error_details_copied")))
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
            ? t("selection.k_path_copied")
            : field === "filename"
              ? t("selection.k_filename_copied")
              : t("selection.k_selection_copied"),
        ),
      )
      .catch(report);
  };
  const reveal = (reference: string, containing = false) => {
    const operation = containing
      ? t("app.k_open_containing_folder")
      : t("app.k_reveal_in_file_manager");
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
    setAncestorOwner(null);
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
        setResultContext({ confirmation, ancestorOwner });
        setResults(value);
        setSelected(new Set());
        runScan(api.refresh(), view, false);
      })
      .catch(report)
      .finally(() => setActing(false));
  };
  const recoveryTargets = forceRecoveryTargets(
    resultContext?.confirmation ?? null,
    results ?? [],
  );
  const adminRecovery =
    results?.some((result) => result.admin_recovery) &&
    !resultContext?.confirmation.elevated;
  const prepareAdminRecovery = () => {
    if (!resultContext || acting) return;
    setActing(true);
    void api
      .prepareElevated(resultContext.confirmation.ticket)
      .then((value) => {
        setActing(false);
        setResults(null);
        setConfirmation(value);
      })
      .catch(report)
      .finally(() => setActing(false));
  };
  const prepareForceRecovery = () => {
    if (!resultContext || acting || status.scanning || !recoveryTargets.length)
      return;
    setActing(true);
    const owner = resultContext.ancestorOwner;
    const request = owner
      ? api.prepareAncestor(owner, recoveryTargets[0].key, true)
      : api.prepare(
          status.revision,
          recoveryTargets.map((target) => target.key),
          true,
        );
    void request
      .then((value) => {
        // Enable Cancel before the confirmation mounts and chooses its default focus.
        setActing(false);
        setAncestorOwner(owner);
        setResults(null);
        setConfirmation(value);
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
      } else if (!editing && command && event.key.toLowerCase() === "b") {
        event.preventDefault();
        toggleSidebarCollapsed();
      } else if (command && (event.key === "+" || event.key === "=")) {
        event.preventDefault();
        adjustZoom(1);
      } else if (command && event.key === "-") {
        event.preventDefault();
        adjustZoom(-1);
      } else if (command && event.key === "0") {
        event.preventDefault();
        setFontSize(14);
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
        if (maximized) setMaximized(false);
        else {
          setSelected(new Set());
          setFocused(null);
        }
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
    if (next === "settings" || next === "history") setMaximized(false);
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
          <span>{t("app.k_desktop")}</span>
          <span className="rc-badge">RC</span>
        </div>
        <div className="header-actions">
          {status.revision > 0 && (
            <>
              <label className="auto-refresh-control">
                <Timer size={14} />
                <span>{t("app.k_auto")}</span>
                <select
                  aria-label={t("status.k_automatic_refresh_interval")}
                  value={autoReloadSeconds}
                  onChange={(event) =>
                    setAutoReloadSeconds(Number(event.target.value))
                  }
                >
                  {autoReloadOptions.map((seconds) => (
                    <option key={seconds} value={seconds}>
                      {seconds === 0 ? t("common.k_off") : `${seconds}s`}
                    </option>
                  ))}
                </select>
              </label>
              <details className="auto-refresh-info">
                <summary
                  aria-label={t("status.k_automatic_refresh_information")}
                  title={t("status.k_about_automatic_refresh")}
                >
                  <Info size={15} />
                </summary>
                <div role="note">
                  <strong>{t("status.k_automatic_refresh")}</strong>
                  <p>
                    {t(
                      "inspection.k_off_by_default_choose_an_interval_to_re_fb2f1749",
                    )}
                  </p>
                  <p>
                    {t(
                      "status.k_it_can_help_with_changing_processes_or_f5538b6f",
                    )}
                  </p>
                </div>
              </details>
            </>
          )}
          <button
            disabled={!status.revision}
            onClick={refresh}
            title={`${t("status.k_refresh")} (${modifier}+R ${t("common.k_or")} F5)`}
          >
            <RefreshCw size={14} className={status.scanning ? "spin" : ""} />
            {t("status.k_refresh")}
            <kbd className="shortcut" aria-hidden="true">
              F5
            </kbd>
          </button>
        </div>
      </header>
      <div className={`app-body${maximized ? " grid-maximized" : ""}`}>
        <nav
          className={`sidebar${sidebarIsCollapsed ? " collapsed" : ""}`}
          aria-label={t("navigation.k_workspace")}
        >
          {!maximized && (
            <button
              className="sidebar-toggle"
              aria-expanded={!sidebarCollapsed}
              title={
                sidebarCollapsed
                  ? t("navigation.k_expand_sidebar")
                  : t("navigation.k_collapse_sidebar")
              }
              onClick={toggleSidebarCollapsed}
            >
              {sidebarCollapsed ? (
                <PanelLeftOpen size={16} />
              ) : (
                <PanelLeftClose size={16} />
              )}
              <span className="nav-label">
                {sidebarCollapsed
                  ? t("navigation.k_expand_sidebar")
                  : t("navigation.k_collapse_sidebar")}
              </span>
            </button>
          )}
          <div className="nav-section">
            {t("navigation.k_workspace_70398828")}
          </div>
          <button
            title={`${t("navigation.k_processes")} (${modifier}+1)`}
            className={view === "processes" ? "active" : ""}
            onClick={() => changeView("processes")}
          >
            <Activity size={17} />
            <span className="nav-label">{t("navigation.k_processes")}</span>
            <span className="nav-badge">{status.processes || ""}</span>
            {shortcut("1")}
          </button>
          <button
            title={`${t("status.k_file_usages")} (${modifier}+2)`}
            className={view === "handles" ? "active" : ""}
            onClick={() => changeView("handles")}
          >
            <Files size={17} />
            <span className="nav-label">{t("status.k_file_usages")}</span>
            <span className="nav-badge">{status.usages || ""}</span>
            {shortcut("2")}
          </button>
          <button
            title={`${t("navigation.k_ports")} (${modifier}+3)`}
            className={view === "ports" ? "active" : ""}
            onClick={() => changeView("ports")}
          >
            <Network size={17} />
            <span className="nav-label">{t("navigation.k_ports")}</span>
            <span className="nav-badge">{status.ports || ""}</span>
            {shortcut("3")}
          </button>
          <button
            title={`${t("history.k_recent_targets")} (${modifier}+4)`}
            className={view === "history" ? "active" : ""}
            onClick={() => changeView("history")}
          >
            <History size={17} />
            <span className="nav-label">{t("history.k_recent_targets")}</span>
            {shortcut("4")}
          </button>
          <div className="sidebar-rule" />
          <div className="nav-section">{t("inspection.k_inspect_target")}</div>
          <button
            title={t("inspection.k_open_file")}
            onClick={() => runScan(api.choose(false))}
          >
            <File size={16} />
            <span className="nav-label">{t("inspection.k_open_file")}</span>
          </button>
          <button
            title={t("inspection.k_open_folder")}
            onClick={() => runScan(api.choose(true))}
          >
            <FolderOpen size={17} />
            <span className="nav-label">{t("inspection.k_open_folder")}</span>
          </button>
          <div className="sidebar-bottom">
            <p>
              {t("app.k_know_what_s_using")}
              <br />
              {t("app.k_your_files")}
            </p>
            <SettingsMenu
              updates={updates}
              openAbout={() => setShowAbout(true)}
              theme={theme}
              appliedTheme={appliedTheme}
              onThemeChange={setTheme}
              active={view === "settings"}
              openSettings={() => changeView("settings")}
            />
          </div>
        </nav>
        <main>
          {maximized && (
            <div className="maximize-banner" role="status">
              <Maximize2 size={14} />
              <span>{t("table.k_focus_mode_hint", { shortcut: "Esc" })}</span>
              <button onClick={() => setMaximized(false)}>
                {t("table.k_restore_layout")}
              </button>
            </div>
          )}
          {error && (
            <div className="error-banner" role="alert">
              <ShieldAlert size={17} />
              <div className="error-content">
                <div className="error-summary">
                  <strong>
                    {t("diagnostics.k_operation_could_not_complete")}
                  </strong>
                  <p>{error}</p>
                </div>
                <div className="error-actions">
                  <button
                    onClick={() => setShowErrorDetails((visible) => !visible)}
                    aria-expanded={showErrorDetails}
                  >
                    {showErrorDetails
                      ? t("common.k_hide_details")
                      : t("common.k_details")}
                  </button>
                  <button onClick={openIssueReport}>
                    <ExternalLink size={13} /> {t("diagnostics.k_open_issue")}
                  </button>
                  <span>
                    {t(
                      "diagnostics.k_review_the_draft_and_remove_private_pat_831914fd",
                    )}
                  </span>
                </div>
              </div>
              <button
                className="icon-button"
                aria-label={t("app.k_dismiss_error")}
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
              {!maximized && (
                <div className="workspace-heading">
                  <div>
                    <div className="eyebrow">
                      {view === "ports"
                        ? t("inspection.k_network_inspection")
                        : t("inspection.k_file_inspection")}
                    </div>
                    <h1>
                      {view === "ports"
                        ? t("inspection.k_local_ports")
                        : view === "handles"
                          ? t("status.k_file_usages")
                          : t("navigation.k_processes")}
                    </h1>
                    <p>
                      {view === "ports"
                        ? t(
                            "inspector.k_find_local_tcp_listeners_bound_udp_sock_7d6edd16",
                          )
                        : status.target
                          ? t(
                              "inspection.k_processes_referencing_your_target_and_i_9e70e943",
                            )
                          : t(
                              "inspection.k_find_out_which_processes_are_using_a_fi_c1cc0eee",
                            )}
                    </p>
                  </div>
                  <button
                    className="primary"
                    onClick={() => runScan(api.choose(true))}
                  >
                    <FolderOpen size={15} />
                    {t("inspection.k_open_folder")}
                  </button>
                </div>
              )}
              {!maximized && view !== "ports" && (
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
                    aria-label={t("inspection.k_target_file_or_folder_path")}
                    placeholder={t("inspection.k_paste_a_file_or_folder_path")}
                    value={path}
                    onChange={(event) => {
                      setPath(event.target.value);
                      setPathEdited(true);
                    }}
                    spellCheck={false}
                  />
                  <button type="submit" disabled={!path.trim()}>
                    {t("inspection.k_inspect")}
                    <ArrowRight size={14} />
                  </button>
                </form>
              )}
              {view !== "ports" && !status.target && !status.scanning ? (
                <div className="welcome">
                  <div className="welcome-icon">
                    <FileSearch size={36} />
                  </div>
                  <h2>{t("inspection.k_a_clear_view_of_files_in_use")}</h2>
                  <p>
                    {t(
                      "inspection.k_drop_a_file_or_folder_anywhere_in_this_window",
                    )}
                    <br />
                    {t(
                      "inspection.k_oflh_will_find_the_processes_referencing_it",
                    )}
                  </p>
                  <div className="welcome-actions">
                    <button
                      className="primary"
                      onClick={() => runScan(api.choose(false))}
                    >
                      <File size={15} />
                      {t("inspection.k_choose_file")}
                    </button>
                    <button onClick={() => runScan(api.choose(true))}>
                      <FolderOpen size={15} />
                      {t("inspection.k_choose_folder")}
                    </button>
                  </div>
                  <div className="welcome-note">
                    <ShieldAlert size={15} />
                    <span>
                      {t(
                        "inspection.k_open_files_do_not_necessarily_mean_locked_files",
                      )}
                      <br />
                      {t(
                        "filters.k_oflh_shows_lock_evidence_when_the_opera_adfdf7ef",
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
                        aria-label={t("search.k_search_loaded_results")}
                        title={t(
                          "search.k_search_shortcut_f_or_escape_returns_to_e2053cec",
                          { shortcut: modifier },
                        )}
                        placeholder={
                          view === "ports"
                            ? t("search.k_search_ports_e_g_80_port_8080_tcp")
                            : t("search.k_search_names_pids_paths")
                        }
                        value={query}
                        onChange={(event) => setQuery(event.target.value)}
                      />
                      {!query && shortcut("F")}
                      {query && (
                        <button
                          className="icon-button"
                          aria-label={t("search.k_clear_search")}
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
                        {t("inspection.k_target_processes_only")}
                      </label>
                    ) : (
                      <label className="checkbox-label">
                        <input
                          type="checkbox"
                          checked={locks}
                          onChange={(event) => setLocks(event.target.checked)}
                        />
                        {t("filters.k_lock_evidence_only")}
                      </label>
                    )}

                    <button className="select-all" onClick={selectAll}>
                      {t("selection.k_select_all")}
                    </button>
                    <details className="column-picker">
                      <summary>
                        <Columns3 size={14} />
                        {t("table.k_columns")}
                        <ChevronDown
                          size={12}
                          className="column-picker-caret"
                        />
                      </summary>
                      <div
                        className="column-picker-menu"
                        role="group"
                        aria-label={t("table.k_visible_columns")}
                      >
                        <span className="column-picker-heading">
                          {t("table.k_show_in_grid")}
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
                          ? t("filters.k_clear_applied_filters_before_closing")
                          : t("filters.k_open_column_filters")
                      }
                      onClick={() => setShowColumns((visible) => !visible)}
                    >
                      <SlidersHorizontal size={14} />
                      {t("filters.k_column_filters")}
                      {columnCount ? ` (${columnCount})` : ""}
                    </button>
                    <span className="muted result-count">
                      {total} {t("app.k_results")}
                    </span>
                    <button
                      className="icon-button"
                      aria-pressed={maximized}
                      aria-label={
                        maximized
                          ? t("table.k_restore_layout")
                          : t("table.k_maximize_grid")
                      }
                      title={
                        maximized
                          ? t("table.k_restore_layout")
                          : t("table.k_maximize_grid")
                      }
                      onClick={() => setMaximized((value) => !value)}
                    >
                      {maximized ? (
                        <Minimize2 size={14} />
                      ) : (
                        <Maximize2 size={14} />
                      )}
                    </button>
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
                      aria-label={t("filters.k_applied_column_filters")}
                    >
                      <strong>
                        <SlidersHorizontal size={14} />
                        {t("filters.k_filters_active")}
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
                        {t("filters.k_clear_filters")}
                      </button>
                    </div>
                  )}
                  {query && view !== "ports" && (
                    <div className="search-explanation">
                      {t(
                        "filters.k_search_includes_full_paths_a_shared_fol_1d2d885e",
                      )}
                    </div>
                  )}
                  {scope && (
                    <div className="scope-bar">
                      {t("app.k_showing")}{" "}
                      {view === "ports"
                        ? t("inspection.k_local_ports_903c8be8")
                        : t("status.k_file_usages_d01933d6")}{" "}
                      for <strong>{scope.name}</strong>
                      <button onClick={() => setScope(null)}>
                        <X size={12} />
                        {t("filters.k_clear_process_filter")}
                      </button>
                    </div>
                  )}
                  {selected.size > 0 ? (
                    <div className="selection-toolbar">
                      <strong>
                        {selected.size}{" "}
                        {selected.size === 1
                          ? t("app.k_process_c2e2d662")
                          : t("navigation.k_processes_da2c4eba")}{" "}
                        {t("selection.k_selected")}
                      </strong>
                      <span className="muted">
                        {t(
                          "selection.k_may_include_processes_outside_this_view",
                        )}
                      </span>
                      <button onClick={() => copy()}>
                        <Copy size={13} />
                        {t("selection.k_copy")}
                      </button>
                      <button disabled={acting} onClick={() => prepare(false)}>
                        {t("termination.k_terminate")}
                      </button>
                      <button
                        className="danger-text"
                        disabled={acting}
                        onClick={() => prepare(true)}
                      >
                        {t("termination.k_force_terminate")}
                      </button>
                      <button
                        className="icon-button"
                        aria-label={t("selection.k_clear_selection")}
                        onClick={() => setSelected(new Set())}
                      >
                        <X size={14} />
                      </button>
                    </div>
                  ) : (
                    <div className="selection-toolbar selection-hint">
                      {t(
                        "selection.k_click_a_row_to_inspect_use_the_panel_bu_1ffc65e8",
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
                              "inspection.k_wait_for_the_current_scan_to_finish_bef_20cd41dd",
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
                          setAncestorOwner(details.process.process_key);
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
                      {t(
                        view === "ports"
                          ? "termination.k_local_bindings_do_not_prove_external_re_a5cdb1ad"
                          : "app.k_file_usage_is_not_proof_of_a_lock_windo_f7640915",
                      )}
                    </span>
                    {status.warnings.length > 0 && (
                      <details>
                        <summary>
                          {status.warnings.length}{" "}
                          {t("status.k_coverage_notices")}
                        </summary>
                        <ul>
                          {status.warnings.map((warning, index) => (
                            <li key={index}>{tValue(warning)}</li>
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
              <span className="eyebrow">{t("history.k_saved_history")}</span>
              <h1>{t("history.k_recent_targets")}</h1>
              <p className="muted">
                {t("history.k_reinspect_targets_saved_on_this_device_b58bafd9")}
              </p>
              {recent.length ? (
                <>
                  <div className="recent-toolbar">
                    <label className="recent-search">
                      <Search size={15} />
                      <input
                        type="search"
                        aria-label={t("history.k_search_recent_targets")}
                        placeholder={t("history.k_filter_recent_targets")}
                        value={recentQuery}
                        onChange={(event) => setRecentQuery(event.target.value)}
                      />
                    </label>
                    <span className="muted recent-count">
                      {filteredRecent.length} {t("app.k_of")} {recent.length}
                    </span>
                    <button
                      className="danger-text"
                      disabled={!recent.length}
                      onClick={clearRecent}
                    >
                      <Trash2 size={14} />
                      {t("app.k_clear_all")}
                    </button>
                  </div>
                  {filteredRecent.length ? (
                    <div
                      className="recent-list"
                      aria-label={t("history.k_recent_targets")}
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
                            aria-label={`${t("app.k_remove")} ${target.display}`}
                            title={t("history.k_remove_from_recent_targets")}
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
                      <h3>{t("history.k_no_matching_recent_targets")}</h3>
                      <button onClick={() => setRecentQuery("")}>
                        {t("search.k_clear_search")}
                      </button>
                    </div>
                  )}
                </>
              ) : (
                <div className="empty">
                  <History size={30} />
                  <h3>{t("history.k_no_recent_targets")}</h3>
                  <p>
                    {t(
                      "inspection.k_choose_a_file_or_folder_to_start_an_inspection",
                    )}
                  </p>
                </div>
              )}
            </div>
          ) : (
            <div className="content-page">
              <span className="eyebrow">{t("app.k_preferences")}</span>
              <h1>{t("navigation.k_settings")}</h1>
              <section className="setting-section">
                <div>
                  <h3>{t("settings.k_appearance")}</h3>
                  <p className="muted">
                    {t("themes.k_choose_a_theme_or_follow_your_system")}
                  </p>
                </div>
                <ThemePicker
                  theme={theme}
                  appliedTheme={appliedTheme}
                  onChange={setTheme}
                />
              </section>
              <section className="setting-section">
                <div>
                  <h3>{t("language.k_language")}</h3>
                  <p className="muted">
                    {t(
                      "language.k_choose_a_language_or_follow_your_system_setting",
                    )}
                  </p>
                </div>
                <div className="font-setting">
                  <label htmlFor="language">{t("language.k_language")}</label>
                  <select
                    id="language"
                    value={localePreference}
                    onChange={(event) => {
                      try {
                        localStorage.setItem(
                          "oflh-language",
                          event.target.value,
                        );
                      } catch {
                        // Apply the language for this session even without storage.
                      }
                      window.location.reload();
                    }}
                  >
                    {languageOptions.map(({ value, label }) => (
                      <option key={value} value={value}>
                        {t(label)}
                      </option>
                    ))}
                  </select>
                </div>
              </section>
              <section className="setting-section">
                <div>
                  <h3>{t("settings.k_interface_font_size")}</h3>
                  <p className="muted">
                    {t(
                      "inspector.k_scale_text_throughout_the_workspace_inc_945ceeb1",
                    )}
                  </p>
                </div>
                <div className="font-setting">
                  <label htmlFor="font-size">{t("app.k_size")}</label>
                  <select
                    id="font-size"
                    value={fontSize}
                    onChange={(event) =>
                      setFontSize(Number(event.target.value))
                    }
                  >
                    {[12, 13, 14, 15, 16, 17, 18, 20, 22, 24].map((size) => (
                      <option key={size} value={size}>
                        {size} px
                      </option>
                    ))}
                  </select>
                  <button onClick={() => setFontSize(14)}>
                    {t("settings.k_reset_to_default")}
                  </button>
                </div>
              </section>
              <UpdateBanner {...updates} />
              <section className="setting-section">
                <div>
                  <h3>{t("settings.k_interface_zoom")}</h3>
                  <p className="muted">
                    {t("settings.k_scale_the_whole_interface_zoom_help")}
                  </p>
                </div>
                <div className="font-setting">
                  <button
                    className="icon-button"
                    aria-label={t("settings.k_zoom_out")}
                    title={`${t("settings.k_zoom_out")} (${modifier}+-)`}
                    disabled={fontSize <= 12}
                    onClick={() => adjustZoom(-1)}
                  >
                    <ZoomOut size={16} />
                  </button>
                  <span className="zoom-value">{zoomPercent}%</span>
                  <button
                    className="icon-button"
                    aria-label={t("settings.k_zoom_in")}
                    title={`${t("settings.k_zoom_in")} (${modifier}++)`}
                    disabled={fontSize >= 24}
                    onClick={() => adjustZoom(1)}
                  >
                    <ZoomIn size={16} />
                  </button>
                  <button onClick={() => setFontSize(14)}>
                    {t("settings.k_reset_to_default")}
                  </button>
                </div>
              </section>
              <section className="setting-section">
                <div>
                  <h3>{t("settings.k_about_oflh")}</h3>
                  <p>
                    {t("app.k_oflh_is_an_independent_project_by")}{" "}
                    <button
                      className="inline-link"
                      onClick={() => void api.openProfile().catch(report)}
                    >
                      Karim Zouine
                    </button>
                    {t(
                      "app.k_built_in_spare_time_there_is_no_company_5cd7cc08",
                    )}
                  </p>
                  <p className="muted">
                    {t(
                      "support.k_if_oflh_helps_your_work_you_can_support_4c5c07a8",
                    )}
                  </p>
                </div>
                <div className="inline-actions about-links">
                  <button onClick={() => void api.openProject().catch(report)}>
                    <ExternalLink size={14} />{" "}
                    {t("app.k_view_project_on_github")}
                  </button>
                  {!/^\d+\.\d+\.\d+$/.test(status.version) &&
                    status.build_url && (
                      <button
                        onClick={() => void api.openBuild().catch(report)}
                      >
                        <ExternalLink size={14} /> {t("app.k_view_rc_pipeline")}
                      </button>
                    )}
                </div>
                <p className="muted about-version">
                  <button
                    className="inline-link"
                    onClick={() =>
                      void api.openInstalledRelease().catch(report)
                    }
                  >
                    {t("app.k_installed_version", { version: status.version })}
                  </button>{" "}
                  · {t("app.k_mit_license")}
                </p>
              </section>
              <section className="setting-section shortcuts">
                <h3>
                  <Keyboard size={17} />
                  {t("settings.k_keyboard_shortcuts")}
                </h3>
                <dl>
                  <dt>
                    {t("history.k_processes_file_usages_ports_recent_targets")}
                  </dt>
                  <dd>{modifier}+1 / 2 / 3 / 4</dd>
                  <dt>{t("navigation.k_next_previous_workspace")}</dt>
                  <dd>Ctrl+Tab / Ctrl+Shift+Tab</dd>
                  <dt>{t("inspector.k_toggle_selected_process_details")}</dt>
                  <dd>{modifier}+Shift+D</dd>
                  <dt>{t("navigation.k_settings")}</dt>
                  <dd>{modifier}+,</dd>
                  <dt>{t("inspection.k_open_file_folder")}</dt>
                  <dd>Ctrl / ⌘ O · {t("app.k_shift_for_folder")}</dd>
                  <dt>{t("search.k_search_results")}</dt>
                  <dd>
                    {modifier}+F {t("common.k_or")} / ·{" "}
                    {t("search.k_escape_leaves_search")}
                  </dd>
                  <dt>{t("inspection.k_refresh_target")}</dt>
                  <dd>Ctrl / ⌘ R {t("common.k_or")} F5</dd>
                  <dt>{t("selection.k_select_all_matching_processes")}</dt>
                  <dd>Ctrl / ⌘ A</dd>
                  <dt>{t("selection.k_copy_selected_processes")}</dt>
                  <dd>Ctrl / ⌘ C</dd>
                  <dt>{t("app.k_navigate_toggle_selection")}</dt>
                  <dd>↑ ↓ / Space</dd>
                  <dt>{t("app.k_context_actions")}</dt>
                  <dd>Shift F10</dd>
                  <dt>{t("selection.k_clear_selection_and_details")}</dt>
                  <dd>Escape</dd>
                  <dt>{t("settings.k_zoom_in_out_reset")}</dt>
                  <dd>Ctrl / ⌘ + / - / 0</dd>
                </dl>
              </section>
              {import.meta.env.DEV && (
                <section className="setting-section developer-tools">
                  <div>
                    <h3>{t("app.k_developer_options")}</h3>
                    <p className="muted">
                      {t(
                        "diagnostics.k_preview_the_operation_error_details_and_667dc1ef",
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
                    <ShieldAlert size={14} />{" "}
                    {t("diagnostics.k_show_sample_error")}
                  </button>
                </section>
              )}
            </div>
          )}
        </main>
      </div>
      <footer className="statusbar">
        <span className="status-current" role="status">
          {status.scanning && <LoaderCircle size={13} className="spin" />}
          <span className="status-current-label">
            {status.scanning
              ? t("inspection.k_scanning")
              : status.revision
                ? t("inspection.k_inspection_complete")
                : t("inspection.k_ready_to_inspect")}
          </span>
          {status.scanning && (
            <button
              className="status-cancel"
              onClick={() => void api.cancel().then(apply).catch(report)}
            >
              {t("common.k_cancel")}
            </button>
          )}
        </span>
        <span className="status-metrics">
          {status.processes} {t("status.k_file_users")} · {status.usages}{" "}
          {t("status.k_file_usages_d01933d6")} · {status.ports}{" "}
          {t("navigation.k_ports").toLocaleLowerCase()}
        </span>
        <span className="footer-end">
          <span className="footer-attribution">
            {selected.size
              ? `${selected.size} ${t("selection.k_selected")} · `
              : ""}
            {t("app.k_independent_project_by")}{" "}
            <button
              className="footer-profile"
              title={`${t("app.k_open_cf9b7706")} Karim Zouine ${t("app.k_github_profile")}`}
              onClick={() => void api.openProfile().catch(report)}
            >
              Karim Zouine
            </button>
            , {t("app.k_built_in_spare_time")}
          </span>
          <button
            className="footer-link footer-star"
            title={t("navigation.k_star_on_github")}
            aria-label={t("navigation.k_star_on_github")}
            onClick={() => void api.openProject().catch(report)}
          >
            <Star size={14} aria-hidden="true" />
            <span>{t("navigation.k_star_on_github")}</span>
          </button>
          <button
            className="footer-link"
            title={t("support.k_choose_how_to_support_oflh")}
            onClick={() => setShowSupport(true)}
          >
            {t("support.k_donate")}
          </button>
        </span>
      </footer>
      {dragging && (
        <div className="drop-overlay">
          <div>
            <FolderOpen size={42} />
            <h2>{t("inspection.k_drop_file_or_folder_to_inspect")}</h2>
            <p>{t("inspection.k_one_target_at_a_time")}</p>
          </div>
        </div>
      )}
      {showAbout && (
        <AboutDialog
          status={status}
          close={() => setShowAbout(false)}
          report={report}
        />
      )}
      <UpdateFeedback updates={updates} report={report} />
      {toast && (
        <div className="toast" role="status">
          <Check size={15} />
          {toast}
        </div>
      )}
      {showErrorDetails && error && (
        <Modal
          title={t("common.k_operation_details")}
          close={() => setShowErrorDetails(false)}
        >
          <p className="muted">
            {t(
              "diagnostics.k_diagnostic_details_can_contain_local_pa_5bc47018",
            )}
          </p>
          <pre className="error-details-modal">{errorDetails || error}</pre>
          <div className="modal-actions">
            <button onClick={copyErrorDetails}>
              <Copy size={14} /> {t("selection.k_copy_details")}
            </button>
            <button onClick={() => setShowErrorDetails(false)}>
              {t("common.k_close")}
            </button>
          </div>
        </Modal>
      )}
      {showSupport && (
        <Modal
          title={t("support.k_support_oflh")}
          close={() => setShowSupport(false)}
        >
          <p className="support-intro">
            {t("support.k_support_is_optional_choose_the_route_th_7ee2133e")}
          </p>
          <div className="support-options">
            <section className="support-option">
              <h3>
                <Coffee size={17} /> {t("support.k_buy_me_a_coffee")}
              </h3>
              <p>
                <strong>{t("support.k_good_for")}</strong>{" "}
                {t("support.k_a_simple_contribution_from_an_individual")}
              </p>
              <p className="muted">
                <strong>{t("support.k_trade_off")}</strong>{" "}
                {t("support.k_checkout_is_handled_by_a_separate_service")}
              </p>
              <button
                className="primary"
                onClick={() => chooseSupport("coffee")}
              >
                {t("support.k_continue_with_buy_me_a_coffee")}
                <ExternalLink size={13} />
              </button>
            </section>
            <section className="support-option">
              <h3>
                <Star size={17} /> {t("support.k_github_sponsors")}
              </h3>
              <p>
                <strong>{t("support.k_good_for")}</strong>{" "}
                {t("support.k_ongoing_sponsorship_and_company_support")}
              </p>
              <p className="muted">
                <strong>{t("support.k_trade_off")}</strong>{" "}
                {t("support.k_sponsorship_uses_github_s_checkout_flow")}
              </p>
              <button
                className="primary"
                onClick={() => chooseSupport("sponsors")}
              >
                {t("support.k_continue_to_github_sponsors")}
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
            {t("app.k_actions_for_pid")} {context.pid}
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
                  {t("selection.k_copy_port")}
                </button>
                <button
                  onClick={() => {
                    copy("endpoint", context.port!.reference);
                    setContext(null);
                  }}
                >
                  {t("selection.k_copy_local_endpoint")}
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
              {t("selection.k_copy_path")}
            </button>
            <button
              onClick={() => {
                copy("filename", context.path_ref);
                setContext(null);
              }}
              disabled={!context.path}
            >
              {t("selection.k_copy_filename")}
            </button>
            {context.pid > 0 && (
              <button
                onClick={() => {
                  copy("pid", undefined, [context.process_key]);
                  setContext(null);
                }}
              >
                {t("selection.k_copy_pid")}
              </button>
            )}
            <button
              onClick={() => {
                copy("name", undefined, [context.process_key]);
                setContext(null);
              }}
            >
              {t("selection.k_copy_process_name")}
            </button>
            <button
              onClick={() => {
                reveal(context.path_ref);
                setContext(null);
              }}
              disabled={!context.path}
            >
              <ExternalLink size={15} />
              {t("app.k_reveal_in_file_manager")}
            </button>
            <button
              onClick={() => {
                reveal(context.path_ref, true);
                setContext(null);
              }}
              disabled={!context.path}
            >
              {t("app.k_open_containing_folder")}
            </button>
            <hr />
            <button
              onClick={() => {
                setFocused(context.process_key);
                setContext(null);
              }}
            >
              {t("inspector.k_view_process_details")}
            </button>
            <button
              onClick={() => {
                setView("handles");
                setScope({ key: context.process_key, name: context.name });
                setQuery("");
                setContext(null);
              }}
            >
              {t("inspector.k_view_matching_handles")}
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
                  {t("inspector.k_inspect_owner_folder")}
                </button>
              )}
            {context.actionable && (
              <>
                <hr />
                <button onClick={() => prepare(false, [context.process_key])}>
                  {t("termination.k_terminate")}
                </button>
                <button
                  className="danger-text"
                  onClick={() => prepare(true, [context.process_key])}
                >
                  {t("termination.k_force_terminate")}
                </button>
              </>
            )}
          </div>
        </Modal>
      )}
      {showThemeWelcome && (
        <Modal
          title={t("app.k_make_oflh_yours")}
          close={() => setShowThemeWelcome(false)}
        >
          <p>{t("themes.k_choose_your_workspace_theme_preview_it_12e8b92b")}</p>
          <ThemePicker
            theme={theme}
            appliedTheme={appliedTheme}
            onChange={setTheme}
          />
          <div className="modal-actions">
            <button
              className="primary"
              data-default-focus
              onClick={() => setShowThemeWelcome(false)}
            >
              {t("app.k_apply_theme")}
            </button>
          </div>
        </Modal>
      )}
      {confirmation && (
        <Modal
          title={
            confirmation.force
              ? t("termination.k_force_terminate_processes")
              : t("termination.k_terminate_processes")
          }
          danger={confirmation.force}
          close={dismiss}
        >
          <div className="confirmation-note">
            <ShieldAlert size={22} />
            <p>
              {confirmation.force
                ? t(
                    "termination.k_force_termination_stops_these_processes_9c247038",
                  )
                : t(
                    "termination.k_request_these_processes_to_stop_unsaved_8b80fb41",
                  )}
            </p>
          </div>
          {confirmation.elevated && (
            <p role="status">{t("termination.k_admin_confirmation")}</p>
          )}
          <p>
            <strong>
              {confirmation.targets.length}{" "}
              {confirmation.targets.length === 1
                ? t("app.k_process_c2e2d662")
                : t("navigation.k_processes_da2c4eba")}
            </strong>{" "}
            — {t("filters.k_all_targets_are_listed_below_including_1515b80a")}
          </p>
          <ul className="confirmation-list">
            {confirmation.targets.map((target) => (
              <li key={target.key}>
                <strong>{target.name || t("app.k_unnamed")}</strong>
                <code>PID {target.pid}</code>
              </li>
            ))}
          </ul>
          <p className="hint">
            {t(
              "termination.k_oflh_validates_each_captured_process_id_5e6d1cc4",
            )}
          </p>
          <div className="modal-actions">
            <button
              data-default-focus
              autoFocus
              disabled={acting}
              onClick={dismiss}
            >
              {t("common.k_cancel")}
            </button>
            <button className="danger" disabled={acting} onClick={terminate}>
              {acting && <LoaderCircle size={14} className="spin" />}
              {acting
                ? t("app.k_waiting_for_process_exit")
                : confirmation.force
                  ? t("termination.k_force_terminate_7b6445b3")
                  : t("termination.k_terminate_77517bd0")}
            </button>
          </div>
        </Modal>
      )}
      {results && (
        <Modal
          title={t("termination.k_termination_results")}
          close={() => setResults(null)}
        >
          <p role="status">
            {status.scanning ? (
              <>
                <LoaderCircle size={14} className="spin" />{" "}
                {t("status.k_updating_results")}
              </>
            ) : (
              t("status.k_results_refreshed_exit_checks_use_the_o_d1d684f4")
            )}
          </p>
          <ul className="action-results">
            {results.map((result) => (
              <li key={result.pid}>
                <code>PID {result.pid}</code>
                <span className={result.error ? "danger-text" : ""}>
                  {result.outcome === "exited" ? (
                    <>
                      <Check size={15} /> {t("termination.k_process_exited")}
                    </>
                  ) : result.outcome === "still_running" ? (
                    t(
                      "termination.k_request_sent_still_running_after_1_5_seconds",
                    )
                  ) : result.outcome === "unverified" ? (
                    t("termination.k_request_sent_couldn_t_verify_exit")
                  ) : result.error?.kind === "identity_changed" ? (
                    t(
                      "termination.k_process_changed_or_already_exited_no_te_9b7e90d2",
                    )
                  ) : (
                    result.error?.message ||
                    t("termination.k_termination_request_failed")
                  )}
                  {result.outcome === "unverified" && result.error && (
                    <small>{result.error.message}</small>
                  )}
                </span>
              </li>
            ))}
          </ul>
          {adminRecovery && (
            <p role="status">{t("termination.k_admin_recovery")}</p>
          )}
          {recoveryTargets.length > 0 && !adminRecovery && (
            <p role="status">
              {t("termination.k_normal_termination_recovery")}
            </p>
          )}
          <div className="modal-actions">
            {adminRecovery ? (
              <button
                className="danger"
                onClick={prepareAdminRecovery}
                disabled={acting}
              >
                {t("termination.k_retry_admin")}
              </button>
            ) : recoveryTargets.length > 0 ? (
              <button
                className="danger"
                onClick={prepareForceRecovery}
                disabled={acting || status.scanning}
              >
                {t("termination.k_force_terminate")}
              </button>
            ) : (
              <button onClick={refresh} disabled={status.scanning}>
                <RefreshCw size={14} /> {t("status.k_refresh_again")}
              </button>
            )}
            <button data-default-focus onClick={() => setResults(null)}>
              {t("common.k_done")}
            </button>
          </div>
        </Modal>
      )}
    </div>
  );
}

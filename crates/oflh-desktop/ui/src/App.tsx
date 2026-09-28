import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  Activity,
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
  Keyboard,
  Network,
  RefreshCw,
  Search,
  Settings,
  ShieldAlert,
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
import { Table } from "./Table";
import { ColumnFilterPanel } from "./ColumnFilters";

type View = "ports" | "processes" | "handles" | "history" | "settings";
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
  useEffect(() => {
    document.documentElement.style.fontSize = `${fontSize}px`;
    try {
      localStorage.setItem("oflh-font-size", String(fontSize));
    } catch {
      /* Session setting remains usable. */
    }
  }, [fontSize]);
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
  const [showColumns, setShowColumns] = useState(false);
  const columns = view === "ports" ? portColumns : fileColumns;
  const setColumns = view === "ports" ? setPortColumns : setFileColumns;
  const columnCount = Object.values(columns).filter(
    (value) => value !== undefined && value !== "" && value !== "any",
  ).length;
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
  const [results, setResults] = useState<ActionResult[] | null>(null);
  const [acting, setActing] = useState(false);
  const [dragging, setDragging] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [toast, setToast] = useState("");
  const [total, setTotal] = useState(0);
  const [recent, setRecent] = useState<{ id: number; display: string }[]>([]);
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
    (failure: unknown) => setError(errorMessage(failure)),
    [],
  );
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
  const copy = (field = "rows", reference?: string, keys = [...selected]) => {
    void api
      .copy(status.revision, keys, field, reference)
      .then(() =>
        setToast(
          field === "path"
            ? "Path copied"
            : field === "filename"
              ? "Filename copied"
              : "Selection copied",
        ),
      )
      .catch(report);
  };
  const reveal = (reference: string, containing = false) => {
    void api.reveal(status.revision, reference, containing).catch(report);
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
    <div className="app-shell">
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
          <button
            className="github-link"
            onClick={() => void api.openProject().catch(report)}
          >
            <Star size={14} /> Star on GitHub <ExternalLink size={12} />
          </button>
          <button
            disabled={!status.revision}
            onClick={refresh}
            title={`Refresh (${modifier}+R or F5)`}
          >
            <RefreshCw size={14} className={status.scanning ? "spin" : ""} />
            Refresh
            <kbd className="shortcut" aria-hidden="true">
              F5
            </kbd>
          </button>
        </div>
      </header>
      <div className="app-body">
        <nav className="sidebar" aria-label="Workspace">
          <div className="nav-section">WORKSPACE</div>
          <button
            title={`Processes (${modifier}+1)`}
            className={view === "processes" ? "active" : ""}
            onClick={() => changeView("processes")}
          >
            <Activity size={17} />
            Processes<span>{status.processes || ""}</span>
            {shortcut("1")}
          </button>
          <button
            title={`File usages (${modifier}+2)`}
            className={view === "handles" ? "active" : ""}
            onClick={() => changeView("handles")}
          >
            <Files size={17} />
            File usages<span>{status.usages || ""}</span>
            {shortcut("2")}
          </button>
          <button
            title={`Ports (${modifier}+3)`}
            className={view === "ports" ? "active" : ""}
            onClick={() => changeView("ports")}
          >
            <Network size={17} />
            Ports<span>{status.ports || ""}</span>
            {shortcut("3")}
          </button>
          <button
            title={`Recent targets (${modifier}+4)`}
            className={view === "history" ? "active" : ""}
            onClick={() => changeView("history")}
          >
            <History size={17} />
            Recent targets{shortcut("4")}
          </button>
          <div className="sidebar-rule" />
          <div className="nav-section">INSPECT TARGET</div>
          <button onClick={() => runScan(api.choose(false))}>
            <File size={16} />
            Open file
          </button>
          <button onClick={() => runScan(api.choose(true))}>
            <FolderOpen size={17} />
            Open folder
          </button>
          <div className="sidebar-bottom">
            <p>
              Know what’s using
              <br />
              your files.
            </p>
            <button
              className={view === "settings" ? "active" : ""}
              onClick={() => changeView("settings")}
            >
              <Settings size={17} />
              Settings
            </button>
            <button
              onClick={() => void api.donate().catch(report)}
              title="Support OFLH on Buy Me a Coffee"
            >
              <Coffee size={16} /> Donate
              <ExternalLink size={12} />
            </button>
            <span className="version">
              {status.version ? `v${status.version}` : "OFLH Desktop"}
            </span>
          </div>
        </nav>
        <main>
          {error && (
            <div className="error-banner" role="alert">
              <ShieldAlert size={17} />
              <div>
                <strong>Operation could not complete</strong>
                <p>{error}</p>
              </div>
              <button
                className="icon-button"
                aria-label="Dismiss error"
                onClick={() => setError(null)}
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
                      ? "NETWORK INSPECTION"
                      : "FILE INSPECTION"}
                  </div>
                  <h1>
                    {view === "ports"
                      ? "Local ports"
                      : view === "handles"
                        ? "File usages"
                        : "Processes"}
                  </h1>
                  <p>
                    {view === "ports"
                      ? "Find local TCP listeners, bound UDP sockets and their captured owners."
                      : status.target
                        ? "Processes referencing your target and its contents."
                        : "Find out which processes are using a file or folder."}
                  </p>
                </div>
                <button
                  className="primary"
                  onClick={() => runScan(api.choose(true))}
                >
                  <FolderOpen size={15} />
                  Open folder
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
                    aria-label="Target file or folder path"
                    placeholder="Paste a file or folder path…"
                    value={path}
                    onChange={(event) => {
                      setPath(event.target.value);
                      setPathEdited(true);
                    }}
                    spellCheck={false}
                  />
                  <button type="submit" disabled={!path.trim()}>
                    Inspect
                    <ArrowRight size={14} />
                  </button>
                </form>
              )}
              {view !== "ports" && !status.target && !status.scanning ? (
                <div className="welcome">
                  <div className="welcome-icon">
                    <FileSearch size={36} />
                  </div>
                  <h2>A clear view of files in use.</h2>
                  <p>
                    Drop a file or folder anywhere in this window.
                    <br />
                    OFLH will find the processes referencing it.
                  </p>
                  <div className="welcome-actions">
                    <button
                      className="primary"
                      onClick={() => runScan(api.choose(false))}
                    >
                      <File size={15} />
                      Choose file
                    </button>
                    <button onClick={() => runScan(api.choose(true))}>
                      <FolderOpen size={15} />
                      Choose folder
                    </button>
                  </div>
                  <div className="welcome-note">
                    <ShieldAlert size={15} />
                    <span>
                      Open files do not necessarily mean locked files.
                      <br />
                      OFLH shows lock evidence when the operating system
                      provides it.
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
                        aria-label="Search loaded results"
                        title={`Search (${modifier}+F or /); Escape returns to the workspace`}
                        placeholder={
                          view === "ports"
                            ? "Search ports… e.g. 80, port:8080, tcp"
                            : "Search names, PIDs, paths…"
                        }
                        value={query}
                        onChange={(event) => setQuery(event.target.value)}
                      />
                      {!query && shortcut("F")}
                      {query && (
                        <button
                          className="icon-button"
                          aria-label="Clear search"
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
                        Target processes only
                      </label>
                    ) : (
                      <label className="checkbox-label">
                        <input
                          type="checkbox"
                          checked={locks}
                          onChange={(event) => setLocks(event.target.checked)}
                        />
                        Lock evidence only
                      </label>
                    )}

                    <button className="select-all" onClick={selectAll}>
                      Select all
                    </button>
                    <button
                      aria-expanded={showColumns}
                      onClick={() => setShowColumns(!showColumns)}
                    >
                      <SlidersHorizontal size={14} />
                      Column filters{columnCount ? ` (${columnCount})` : ""}
                    </button>
                    <span className="muted result-count">{total} results</span>
                  </div>
                  {showColumns && (
                    <ColumnFilterPanel
                      key={view === "ports" ? "port-filters" : "file-filters"}
                      value={columns}
                      ports={view === "ports"}
                      apply={setColumns}
                      close={() => setShowColumns(false)}
                    />
                  )}
                  {query && view !== "ports" && (
                    <div className="search-explanation">
                      Search includes full paths. A shared folder name can match
                      every row; use the Process name column filter to narrow by
                      name.
                    </div>
                  )}
                  {scope && (
                    <div className="scope-bar">
                      Showing {view === "ports" ? "local ports" : "file usages"}{" "}
                      for <strong>{scope.name}</strong>
                      <button onClick={() => setScope(null)}>
                        <X size={12} />
                        Clear process filter
                      </button>
                    </div>
                  )}
                  {status.scanning && (
                    <div className="scan-bar" role="status">
                      <RefreshCw size={13} className="spin" />
                      {status.revision
                        ? "Scanning… Previous results remain available."
                        : "Inspecting visible processes…"}
                      <button
                        onClick={() =>
                          void api.cancel().then(apply).catch(report)
                        }
                      >
                        Cancel scan
                      </button>
                    </div>
                  )}
                  {selected.size > 0 ? (
                    <div className="selection-toolbar">
                      <strong>
                        {selected.size}{" "}
                        {selected.size === 1 ? "process" : "processes"} selected
                      </strong>
                      <span className="muted">
                        May include processes outside this view
                      </span>
                      <button onClick={() => copy()}>
                        <Copy size={13} />
                        Copy
                      </button>
                      <button disabled={acting} onClick={() => prepare(false)}>
                        Terminate…
                      </button>
                      <button
                        className="danger-text"
                        disabled={acting}
                        onClick={() => prepare(true)}
                      >
                        Force terminate…
                      </button>
                      <button
                        className="icon-button"
                        aria-label="Clear selection"
                        onClick={() => setSelected(new Set())}
                      >
                        <X size={14} />
                      </button>
                    </div>
                  ) : (
                    <div className="selection-toolbar selection-hint">
                      Click a row to inspect · Use the panel button to close
                      details
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
                            <li key={index}>{warning}</li>
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
              <span className="eyebrow">THIS SESSION</span>
              <h1>Recent targets</h1>
              <p className="muted">
                Reinspect a recent target. Paths are kept only until you close
                OFLH.
              </p>
              {recent.length ? (
                <div className="recent-list">
                  {recent.map((target) => (
                    <button
                      key={target.id}
                      onClick={() => runScan(api.revisit(target.id))}
                    >
                      <FolderOpen size={18} />
                      <span className="mono">{target.display}</span>
                      <ArrowRight size={15} />
                    </button>
                  ))}
                </div>
              ) : (
                <div className="empty">
                  <History size={30} />
                  <h3>No recent targets</h3>
                  <p>Choose a file or folder to start an inspection.</p>
                </div>
              )}
            </div>
          ) : (
            <div className="content-page">
              <span className="eyebrow">PREFERENCES</span>
              <h1>Settings</h1>
              <section className="setting-section">
                <div>
                  <h3>Appearance</h3>
                  <p className="muted">Choose a theme or follow your system.</p>
                </div>
                <ThemePicker theme={theme} onChange={setTheme} />
              </section>
              <section className="setting-section">
                <div>
                  <h3>Interface font size</h3>
                  <p className="muted">
                    Scale text throughout the workspace, including tables and
                    process details.
                  </p>
                </div>
                <div className="font-setting">
                  <label htmlFor="font-size">Size</label>
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
                    Reset to default
                  </button>
                </div>
              </section>
              <section className="setting-section">
                <div>
                  <h3>About OFLH</h3>
                  <p>Created by Karim Zouine (karimz1).</p>
                  <p className="muted">
                    OFLH Desktop {status.version} · MIT license
                  </p>
                </div>
                <div className="inline-actions about-links">
                  <button onClick={() => void api.openProject().catch(report)}>
                    <ExternalLink size={14} /> View project on GitHub
                  </button>
                  <button onClick={() => void api.donate().catch(report)}>
                    <Coffee size={14} /> Buy Me a Coffee
                  </button>
                </div>
              </section>
              <section className="setting-section shortcuts">
                <h3>
                  <Keyboard size={17} />
                  Keyboard shortcuts
                </h3>
                <dl>
                  <dt>Processes / File usages / Ports / Recent targets</dt>
                  <dd>{modifier}+1 / 2 / 3 / 4</dd>
                  <dt>Next / previous workspace</dt>
                  <dd>Ctrl+Tab / Ctrl+Shift+Tab</dd>
                  <dt>Toggle selected process details</dt>
                  <dd>{modifier}+Shift+D</dd>
                  <dt>Settings</dt>
                  <dd>{modifier}+,</dd>
                  <dt>Open file / folder</dt>
                  <dd>Ctrl / ⌘ O · Shift for folder</dd>
                  <dt>Search results</dt>
                  <dd>{modifier}+F or / · Escape leaves search</dd>
                  <dt>Refresh target</dt>
                  <dd>Ctrl / ⌘ R or F5</dd>
                  <dt>Select all matching processes</dt>
                  <dd>Ctrl / ⌘ A</dd>
                  <dt>Copy selected processes</dt>
                  <dd>Ctrl / ⌘ C</dd>
                  <dt>Navigate / toggle selection</dt>
                  <dd>↑ ↓ / Space</dd>
                  <dt>Context actions</dt>
                  <dd>Shift F10</dd>
                  <dt>Clear selection and details</dt>
                  <dd>Escape</dd>
                </dl>
              </section>
              <section className="setting-section">
                <h3>Search and inspection</h3>
                <p>
                  Search runs over the loaded Rust snapshot and supports
                  substrings, wildcards (*) and word-boundary abbreviations. In
                  Ports, bare digits match port fragments; port:8080 matches
                  exactly 8080. Combine port queries with tcp, udp, ipv4, ipv6
                  or pid:1234. Refresh performs a new system scan.
                </p>
                <p className="muted">
                  Unknown CPU and memory stay unavailable. Access permissions
                  may hide processes. Normal termination never escalates to
                  force termination. Unsaved work can be lost when stopping a
                  process.
                </p>
              </section>
            </div>
          )}
        </main>
      </div>
      <footer className="statusbar">
        <span>
          {status.scanning && <LoaderCircle size={13} className="spin" />}
          {status.scanning
            ? "Scanning"
            : status.revision
              ? "Inspection complete"
              : "Ready to inspect"}
        </span>
        <span>
          {status.processes} file users · {status.usages} file usages ·{" "}
          {status.ports} ports
        </span>
        <span className="footer-end">
          {selected.size ? `${selected.size} selected · ` : ""}Rust inspection
          engine
        </span>
      </footer>
      {dragging && (
        <div className="drop-overlay">
          <div>
            <FolderOpen size={42} />
            <h2>Drop file or folder to inspect</h2>
            <p>One target at a time</p>
          </div>
        </div>
      )}
      {toast && (
        <div className="toast" role="status">
          <Check size={15} />
          {toast}
        </div>
      )}
      {context && (
        <Modal
          title={context.name || `PID ${context.pid}`}
          close={() => setContext(null)}
        >
          <p className="muted">Actions for PID {context.pid}</p>
          <div className="context-actions">
            {context.port && (
              <>
                <button
                  onClick={() => {
                    copy("port", context.port!.reference);
                    setContext(null);
                  }}
                >
                  Copy port
                </button>
                <button
                  onClick={() => {
                    copy("endpoint", context.port!.reference);
                    setContext(null);
                  }}
                >
                  Copy local endpoint
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
              Copy path
            </button>
            <button
              onClick={() => {
                copy("filename", context.path_ref);
                setContext(null);
              }}
              disabled={!context.path}
            >
              Copy filename
            </button>
            {context.pid > 0 && (
              <button
                onClick={() => {
                  copy("pid", undefined, [context.process_key]);
                  setContext(null);
                }}
              >
                Copy PID
              </button>
            )}
            <button
              onClick={() => {
                copy("name", undefined, [context.process_key]);
                setContext(null);
              }}
            >
              Copy process name
            </button>
            <button
              onClick={() => {
                reveal(context.path_ref);
                setContext(null);
              }}
              disabled={!context.path}
            >
              <ExternalLink size={15} />
              Reveal in file manager
            </button>
            <button
              onClick={() => {
                reveal(context.path_ref, true);
                setContext(null);
              }}
              disabled={!context.path}
            >
              Open containing folder
            </button>
            <hr />
            <button
              onClick={() => {
                setFocused(context.process_key);
                setContext(null);
              }}
            >
              View process details
            </button>
            <button
              onClick={() => {
                setView("handles");
                setScope({ key: context.process_key, name: context.name });
                setQuery("");
                setContext(null);
              }}
            >
              View matching handles
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
                  Inspect owner folder
                </button>
              )}
            {context.actionable && (
              <>
                <hr />
                <button onClick={() => prepare(false, [context.process_key])}>
                  Terminate…
                </button>
                <button
                  className="danger-text"
                  onClick={() => prepare(true, [context.process_key])}
                >
                  Force terminate…
                </button>
              </>
            )}
          </div>
        </Modal>
      )}
      {showThemeWelcome && (
        <Modal title="Make OFLH yours" close={() => setShowThemeWelcome(false)}>
          <p>
            Choose your workspace theme. Preview it now; you can change it
            anytime in Settings.
          </p>
          <ThemePicker theme={theme} onChange={setTheme} />
          <div className="modal-actions">
            <button
              className="primary"
              data-default-focus
              onClick={() => setShowThemeWelcome(false)}
            >
              Start inspecting
            </button>
          </div>
        </Modal>
      )}
      {confirmation && (
        <Modal
          title={
            confirmation.force
              ? "Force terminate processes?"
              : "Terminate processes?"
          }
          danger={confirmation.force}
          close={dismiss}
        >
          <div className="confirmation-note">
            <ShieldAlert size={22} />
            <p>
              {confirmation.force
                ? "Force termination stops these processes without allowing normal cleanup. Unsaved work may be lost."
                : "Request these processes to stop. Unsaved work may be lost. This will not escalate to force termination."}
            </p>
          </div>
          <p>
            <strong>
              {confirmation.targets.length}{" "}
              {confirmation.targets.length === 1 ? "process" : "processes"}
            </strong>{" "}
            — all targets are listed below, including any hidden by filters.
          </p>
          <ul className="confirmation-list">
            {confirmation.targets.map((target) => (
              <li key={target.key}>
                <strong>{target.name || "(unnamed)"}</strong>
                <code>PID {target.pid}</code>
              </li>
            ))}
          </ul>
          <p className="hint">
            OFLH validates each captured process identity again before sending
            the request.
          </p>
          <div className="modal-actions">
            <button
              data-default-focus
              autoFocus
              disabled={acting}
              onClick={dismiss}
            >
              Cancel
            </button>
            <button className="danger" disabled={acting} onClick={terminate}>
              {acting && <LoaderCircle size={14} className="spin" />}
              {acting
                ? "Waiting for process exit…"
                : confirmation.force
                  ? "Force terminate"
                  : "Terminate"}
            </button>
          </div>
        </Modal>
      )}
      {results && (
        <Modal title="Termination results" close={() => setResults(null)}>
          <p role="status">
            {status.scanning ? (
              <>
                <LoaderCircle size={14} className="spin" /> Updating results…
              </>
            ) : (
              "Results refreshed. Exit checks use the original process identity."
            )}
          </p>
          <ul className="action-results">
            {results.map((result) => (
              <li key={result.pid}>
                <code>PID {result.pid}</code>
                <span className={result.error ? "danger-text" : ""}>
                  {result.outcome === "exited" ? (
                    <>
                      <Check size={15} /> Process exited
                    </>
                  ) : result.outcome === "still_running" ? (
                    "Request sent · still running after 1.5 seconds"
                  ) : result.outcome === "unverified" ? (
                    "Request sent · couldn’t verify exit"
                  ) : result.error?.kind === "identity_changed" ? (
                    "Process changed or already exited. No termination was sent."
                  ) : (
                    result.error?.message || "Termination request failed"
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
              <RefreshCw size={14} /> Refresh again
            </button>
            <button data-default-focus onClick={() => setResults(null)}>
              Done
            </button>
          </div>
        </Modal>
      )}
    </div>
  );
}

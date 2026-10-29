import {
  AppWindow,
  Check,
  ChevronDown,
  ChevronUp,
  Cog,
  Copy,
  FileText,
  Network,
  Search,
  Workflow,
  type LucideIcon,
} from "lucide-react";
import { useEffect, useId, useRef, useState, type ReactNode } from "react";
import { api, type CappedList, type ConfirmationTarget } from "./api";
import { t, type MessageKey } from "./i18n";
import {
  IMPACT_SENTENCES,
  impactGroupView,
  impactRow,
  impactSummary,
  listTotal,
  scrollFades,
} from "./termination";

type Kind = "windows" | "children" | "files" | "ports";

const GROUPS: Record<Kind, { icon: LucideIcon; heading: MessageKey }> = {
  windows: { icon: AppWindow, heading: "termination.k_impact_group_windows" },
  children: { icon: Workflow, heading: "termination.k_impact_group_children" },
  files: { icon: FileText, heading: "termination.k_impact_group_files" },
  ports: { icon: Network, heading: "termination.k_impact_group_ports" },
};

function CopyButton({ value }: { value: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <button
      type="button"
      className="icon-button impact-copy"
      aria-label={t("termination.k_impact_copy", { value })}
      title={t("termination.k_impact_copy", { value })}
      onClick={() =>
        void api.copyDiagnostic(value).then(
          () => setCopied(true),
          () => setCopied(false),
        )
      }
    >
      {copied ? (
        <Check size={12} aria-hidden="true" />
      ) : (
        <Copy size={12} aria-hidden="true" />
      )}
    </button>
  );
}

function ImpactRow({ kind, item }: { kind: Kind; item: string }) {
  const row = impactRow(kind, item);
  return (
    <li>
      <span className="impact-primary" title={row.primary}>
        {row.primary}
      </span>
      {row.secondary && (
        <span className="impact-secondary" title={row.copy ?? row.secondary}>
          {row.secondary}
        </span>
      )}
      {row.copy && <CopyButton value={row.copy} />}
    </li>
  );
}

/**
 * One Details group: a short preview, then "Show all" expands the rest in
 * place. Long groups get a filter so a hundred window titles stay usable.
 */
export function ImpactGroup({ kind, list }: { kind: Kind; list: CappedList }) {
  const [expanded, setExpanded] = useState(false);
  const [query, setQuery] = useState("");
  const id = useId();
  const total = listTotal(list);
  if (!total) return null;
  const { icon: Icon, heading } = GROUPS[kind];
  const view = impactGroupView(list, expanded, query);
  const toggle = () => {
    setExpanded(!expanded);
    setQuery("");
  };
  return (
    <section className="impact-group" data-expanded={expanded || undefined}>
      <h4>
        <Icon size={13} aria-hidden="true" />
        {t(heading, { count: total })}
      </h4>
      {view.filterable && (
        <div className="impact-filter">
          <Search size={12} aria-hidden="true" />
          <input
            type="search"
            value={query}
            aria-label={t("termination.k_impact_filter")}
            placeholder={t("termination.k_impact_filter_placeholder")}
            onChange={(event) => {
              setQuery(event.target.value);
              // Show matches from the top of the group, not mid-list.
              event.currentTarget
                .closest(".impact-group")
                ?.scrollIntoView({ block: "start" });
            }}
            onKeyDown={(event) => {
              // Escape clears the filter instead of closing the dialog.
              if (event.key === "Escape" && query) {
                event.preventDefault();
                event.stopPropagation();
                setQuery("");
              }
            }}
          />
          <span className="impact-filter-count" aria-live="polite">
            {query
              ? t("termination.k_impact_filter_count", {
                  shown: view.items.length,
                  total: list.items.length,
                })
              : ""}
          </span>
        </div>
      )}
      <ul id={id}>
        {view.items.map(({ item, index }) => (
          <ImpactRow key={index} kind={kind} item={item} />
        ))}
        {view.filterable && query && view.items.length === 0 && (
          <li className="impact-more">
            {t("termination.k_impact_filter_none")}
          </li>
        )}
        {view.unlisted > 0 && (
          <li className="impact-more">
            {t("termination.k_impact_not_listed", { count: view.unlisted })}
          </li>
        )}
      </ul>
      {view.expandable && (
        <button
          type="button"
          className="impact-toggle"
          aria-expanded={expanded}
          aria-controls={id}
          onClick={toggle}
        >
          {expanded ? (
            <ChevronUp size={13} aria-hidden="true" />
          ) : (
            <ChevronDown size={13} aria-hidden="true" />
          )}
          {expanded
            ? t("termination.k_impact_show_less")
            : t("termination.k_impact_show_all", { count: total })}
        </button>
      )}
    </section>
  );
}

/**
 * One confirmation row: the app and up to two plain sentences about what
 * else happens. Technical lists stay in the collapsed Details.
 */
export function TerminationTarget({ target }: { target: ConfirmationTarget }) {
  const parts = impactSummary(target);
  const impact = target.impact;
  return (
    <li>
      <div className="confirmation-target">
        <AppWindow size={16} aria-hidden="true" className="target-icon" />
        <strong>
          {target.name || t("app.k_unnamed")}
          {target.role === "ancestor" && (
            <span className="role-badge">{t("termination.k_role_parent")}</span>
          )}
        </strong>
        <code>PID {target.pid}</code>
      </div>
      {impact && parts.length > 0 && (
        <>
          <p className="impact-summary">
            {parts.slice(0, IMPACT_SENTENCES).map((part, index) => (
              <span
                key={part.key}
                className={part.warn ? "impact-warning" : undefined}
              >
                {index > 0 && " "}
                {t(part.key, part.values)}
              </span>
            ))}
          </p>
          <details className="impact-details">
            <summary>{t("termination.k_impact_details")}</summary>
            {impact.windows ? (
              <ImpactGroup kind="windows" list={impact.windows} />
            ) : (
              <p className="hint">
                {t("termination.k_impact_windows_unavailable")}
              </p>
            )}
            {impact.children && listTotal(impact.children) > 0 && (
              <>
                <ImpactGroup kind="children" list={impact.children} />
                <p className="hint">
                  {t("termination.k_impact_children_note")}
                </p>
              </>
            )}
            <ImpactGroup kind="files" list={impact.files} />
            <ImpactGroup kind="ports" list={impact.ports} />
            {impact.service && (
              <section className="impact-group">
                <h4>
                  <Cog size={13} aria-hidden="true" />
                  {t("termination.k_impact_group_service")}
                </h4>
                <ul>
                  <li>
                    <span className="impact-primary" title={impact.service}>
                      {impact.service}
                    </span>
                    <span className="impact-secondary">
                      {t("termination.k_impact_service_restarts")}
                    </span>
                    <CopyButton value={impact.service} />
                  </li>
                </ul>
              </section>
            )}
            <p className="hint">{t("termination.k_impact_latest_scan")}</p>
          </details>
        </>
      )}
    </li>
  );
}

/**
 * The scrolling target list. Fades at the top and bottom show that more
 * content continues; they disappear at either end and when it all fits.
 */
export function ImpactScrollArea({
  label,
  children,
}: {
  label: string;
  children: ReactNode;
}) {
  const list = useRef<HTMLUListElement>(null);
  const [fades, setFades] = useState({ top: false, bottom: false });
  useEffect(() => {
    const node = list.current;
    if (!node) return;
    const update = () =>
      setFades((old) => {
        const next = scrollFades(node);
        return old.top === next.top && old.bottom === next.bottom ? old : next;
      });
    update();
    node.addEventListener("scroll", update, { passive: true });
    // Expanding Details changes the rows, not the list box, so watch both.
    const resize = new ResizeObserver(update);
    resize.observe(node);
    for (const row of node.children) resize.observe(row);
    node.addEventListener("toggle", update, true);
    return () => {
      node.removeEventListener("scroll", update);
      node.removeEventListener("toggle", update, true);
      resize.disconnect();
    };
  }, []);
  return (
    <div
      className="confirmation-scroll"
      data-fade-top={fades.top || undefined}
      data-fade-bottom={fades.bottom || undefined}
    >
      <ul
        ref={list}
        className="confirmation-list"
        tabIndex={0}
        aria-label={label}
      >
        {children}
      </ul>
    </div>
  );
}

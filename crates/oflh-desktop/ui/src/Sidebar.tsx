import type { ReactNode } from "react";
import {
  Activity,
  ExternalLink,
  File,
  Files,
  FolderOpen,
  History,
  Network,
  PanelLeftClose,
  PanelLeftOpen,
  Settings,
  Star,
} from "lucide-react";
import type { Status } from "./api";

export type View = "ports" | "processes" | "handles" | "history" | "settings";

interface Props {
  view: View;
  status: Status;
  modifier: string;
  shortcut: (keys: string) => ReactNode;
  collapsed: boolean;
  onToggleCollapsed: () => void;
  onChangeView: (next: View) => void;
  onOpenFile: () => void;
  onOpenFolder: () => void;
  onStarGithub: () => void;
}

/** Workspace navigation rail; collapses to icons only to free grid space. */
export function Sidebar({
  view,
  status,
  modifier,
  shortcut,
  collapsed,
  onToggleCollapsed,
  onChangeView,
  onOpenFile,
  onOpenFolder,
  onStarGithub,
}: Props) {
  return (
    <nav
      className={collapsed ? "sidebar collapsed" : "sidebar"}
      aria-label="Workspace"
    >
      <button
        className="sidebar-toggle icon-button"
        aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
        title={collapsed ? "Expand sidebar" : "Collapse sidebar"}
        aria-pressed={collapsed}
        onClick={onToggleCollapsed}
      >
        {collapsed ? <PanelLeftOpen size={16} /> : <PanelLeftClose size={16} />}
      </button>
      <div className="nav-section">WORKSPACE</div>
      <button
        title={`Processes (${modifier}+1)`}
        className={view === "processes" ? "active" : ""}
        onClick={() => onChangeView("processes")}
      >
        <Activity size={17} />
        <span className="sidebar-label">Processes</span>
        <span className="sidebar-count">{status.processes || ""}</span>
        {shortcut("1")}
      </button>
      <button
        title={`File usages (${modifier}+2)`}
        className={view === "handles" ? "active" : ""}
        onClick={() => onChangeView("handles")}
      >
        <Files size={17} />
        <span className="sidebar-label">File usages</span>
        <span className="sidebar-count">{status.usages || ""}</span>
        {shortcut("2")}
      </button>
      <button
        title={`Ports (${modifier}+3)`}
        className={view === "ports" ? "active" : ""}
        onClick={() => onChangeView("ports")}
      >
        <Network size={17} />
        <span className="sidebar-label">Ports</span>
        <span className="sidebar-count">{status.ports || ""}</span>
        {shortcut("3")}
      </button>
      <button
        title={`Recent targets (${modifier}+4)`}
        className={view === "history" ? "active" : ""}
        onClick={() => onChangeView("history")}
      >
        <History size={17} />
        <span className="sidebar-label">Recent targets</span>
        {shortcut("4")}
      </button>
      <div className="sidebar-rule" />
      <div className="nav-section">INSPECT TARGET</div>
      <button title="Open file" onClick={onOpenFile}>
        <File size={16} />
        <span className="sidebar-label">Open file</span>
      </button>
      <button title="Open folder" onClick={onOpenFolder}>
        <FolderOpen size={17} />
        <span className="sidebar-label">Open folder</span>
      </button>
      <div className="sidebar-bottom">
        <p>
          Know what’s using
          <br />
          your files.
        </p>
        <button
          title="Settings"
          className={view === "settings" ? "active" : ""}
          onClick={() => onChangeView("settings")}
        >
          <Settings size={17} />
          <span className="sidebar-label">Settings</span>
        </button>
        <button
          className="github-link"
          title="Star on GitHub"
          onClick={onStarGithub}
        >
          <Star size={14} />
          <span className="sidebar-label">Star on GitHub</span>
          <ExternalLink size={12} />
        </button>
        <span className="version">
          {status.version === "development"
            ? "Development"
            : status.version
              ? `v${status.version}`
              : "OFLH Desktop"}
        </span>
      </div>
    </nav>
  );
}

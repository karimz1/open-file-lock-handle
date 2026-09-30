import { useEffect, useRef, useState } from "react";

const heightStorageKey = "oflh-top-panel-height";
const collapsedStorageKey = "oflh-top-panel-collapsed";
export const topPanelMinHeight = 48;

function readStoredHeight(): number | null {
  try {
    const saved = Number(localStorage.getItem(heightStorageKey));
    return saved >= topPanelMinHeight ? saved : null;
  } catch {
    return null;
  }
}

function readStoredCollapsed(): boolean {
  try {
    return localStorage.getItem(collapsedStorageKey) === "true";
  } catch {
    return false;
  }
}

/**
 * Lets the workspace heading, target bar and filters be dragged shorter or
 * fully collapsed, so the results grid below can use the freed height.
 */
export function useResizableTopPanel() {
  const panelRef = useRef<HTMLDivElement>(null);
  const [height, setHeightState] = useState<number | null>(readStoredHeight);
  const [collapsed, setCollapsed] = useState(readStoredCollapsed);
  const [maxHeight, setMaxHeight] = useState(600);
  const drag = useRef<{ y: number; height: number } | null>(null);

  useEffect(() => {
    const parent = panelRef.current?.parentElement;
    if (!parent) return;
    const observer = new ResizeObserver(() =>
      setMaxHeight(Math.max(topPanelMinHeight, parent.clientHeight - 140)),
    );
    observer.observe(parent);
    return () => observer.disconnect();
  }, []);

  const setHeight = (value: number) => {
    const next = Math.min(maxHeight, Math.max(topPanelMinHeight, value));
    setHeightState(next);
    try {
      localStorage.setItem(heightStorageKey, String(next));
    } catch {
      // The size still applies for the current session.
    }
  };

  const toggleCollapsed = () => {
    setCollapsed((current) => {
      const next = !current;
      try {
        localStorage.setItem(collapsedStorageKey, String(next));
      } catch {
        // The toggle still applies for the current session.
      }
      return next;
    });
  };

  const resetHeight = () => {
    setHeightState(null);
    try {
      localStorage.removeItem(heightStorageKey);
    } catch {
      // The reset still applies for the current session.
    }
  };

  const beginDrag = (clientY: number) => {
    drag.current = {
      y: clientY,
      height:
        panelRef.current?.getBoundingClientRect().height ?? topPanelMinHeight,
    };
  };
  const dragTo = (clientY: number) => {
    if (drag.current) setHeight(drag.current.height + clientY - drag.current.y);
  };
  const endDrag = () => {
    drag.current = null;
  };

  return {
    panelRef,
    collapsed,
    toggleCollapsed,
    height,
    maxHeight,
    setHeight,
    resetHeight,
    beginDrag,
    dragTo,
    endDrag,
  };
}

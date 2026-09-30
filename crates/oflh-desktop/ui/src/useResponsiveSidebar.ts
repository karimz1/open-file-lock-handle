import { useEffect, useState } from "react";

const storageKey = "oflh-sidebar-collapsed";
const narrowWindowQuery = "(max-width: 760px)";

function readStoredPreference(): boolean | null {
  try {
    const saved = localStorage.getItem(storageKey);
    if (saved === "true") return true;
    if (saved === "false") return false;
  } catch {
    // No stored preference; the sidebar keeps following window width.
  }
  return null;
}

function storePreference(collapsed: boolean): void {
  try {
    localStorage.setItem(storageKey, String(collapsed));
  } catch {
    // The toggle still applies for the current session.
  }
}

/**
 * Collapses the sidebar to reclaim grid space on narrow windows.
 *
 * The sidebar follows the window width automatically until the user toggles
 * it manually, at which point their choice is remembered and takes over.
 */
export function useResponsiveSidebar() {
  const [collapsed, setCollapsed] = useState(
    () => readStoredPreference() ?? matchMedia(narrowWindowQuery).matches,
  );
  useEffect(() => {
    const media = matchMedia(narrowWindowQuery);
    const followWindowWidth = () => {
      if (readStoredPreference() === null) setCollapsed(media.matches);
    };
    media.addEventListener("change", followWindowWidth);
    return () => media.removeEventListener("change", followWindowWidth);
  }, []);
  const toggle = () => {
    setCollapsed((current) => {
      const next = !current;
      storePreference(next);
      return next;
    });
  };
  return { collapsed, toggle };
}

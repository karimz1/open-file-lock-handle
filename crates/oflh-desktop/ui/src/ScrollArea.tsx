import { useEffect, useState, type RefObject } from "react";

/** Which edges of a scroll area hide more content (1px slack for zoom rounding). */
export function scrollFades(area: {
  scrollTop: number;
  scrollHeight: number;
  clientHeight: number;
}): { top: boolean; bottom: boolean } {
  const rest = area.scrollHeight - area.clientHeight - area.scrollTop;
  return { top: area.scrollTop > 1, bottom: rest > 1 };
}

/**
 * Calls `update` whenever `node` scrolls, resizes, or its content changes
 * size. Children are watched too because expanding a row grows the content,
 * not the scroll box. Returns the cleanup.
 */
function watch(node: HTMLElement, update: () => void): () => void {
  update();
  node.addEventListener("scroll", update, { passive: true });
  node.addEventListener("toggle", update, true);
  const resize = new ResizeObserver(update);
  const observeChildren = () => {
    resize.disconnect();
    resize.observe(node);
    for (const child of node.children) resize.observe(child);
    update();
  };
  observeChildren();
  const children = new MutationObserver(observeChildren);
  children.observe(node, { childList: true });
  return () => {
    node.removeEventListener("scroll", update);
    node.removeEventListener("toggle", update, true);
    resize.disconnect();
    children.disconnect();
  };
}

/** Fade state for one scroll container, for components that draw their own fades. */
export function useScrollFades(ref: RefObject<HTMLElement | null>) {
  const [fades, setFades] = useState({ top: false, bottom: false });
  useEffect(() => {
    const node = ref.current;
    if (!node) return;
    return watch(node, () =>
      setFades((old) => {
        const next = scrollFades(node);
        return old.top === next.top && old.bottom === next.bottom ? old : next;
      }),
    );
  }, [ref]);
  return fades;
}

/**
 * Every region that can scroll in a small window, and which edges may fade.
 * Tables keep their sticky header crisp, so they only fade at the bottom.
 */
export const SCROLL_REGIONS: { selector: string; top: boolean }[] = [
  { selector: ".table-scroll", top: false },
  { selector: ".inspector", top: true },
  { selector: ".content-page", top: true },
  { selector: ".recent-list", top: true },
  { selector: ".sidebar", top: true },
  { selector: ".column-filters", top: true },
  { selector: ".modal-body", top: true },
  { selector: ".action-results", top: true },
  { selector: ".error-details-modal", top: true },
  { selector: ".evidence-note ul", top: true },
];

function mark(node: HTMLElement, top: boolean) {
  const fades = scrollFades(node);
  node.toggleAttribute("data-fade-top", top && fades.top);
  node.toggleAttribute("data-fade-bottom", fades.bottom);
}

/**
 * Keeps `data-fade-top` / `data-fade-bottom` current on every
 * {@link SCROLL_REGIONS} element in the document, so CSS can fade the edges
 * that hide content. Mount once at the app root.
 */
export function useScrollRegionFades() {
  useEffect(() => {
    const watched = new Map<HTMLElement, () => void>();
    const collect = () => {
      const live = new Set<HTMLElement>();
      for (const { selector, top } of SCROLL_REGIONS) {
        for (const node of document.querySelectorAll<HTMLElement>(selector)) {
          live.add(node);
          if (!watched.has(node))
            watched.set(
              node,
              watch(node, () => mark(node, top)),
            );
        }
      }
      for (const [node, stop] of watched)
        if (!live.has(node)) {
          stop();
          watched.delete(node);
        }
    };
    collect();
    let queued = 0;
    const pages = new MutationObserver(() => {
      if (queued) return;
      queued = requestAnimationFrame(() => {
        queued = 0;
        collect();
      });
    });
    pages.observe(document.body, { childList: true, subtree: true });
    return () => {
      pages.disconnect();
      cancelAnimationFrame(queued);
      for (const stop of watched.values()) stop();
    };
  }, []);
}

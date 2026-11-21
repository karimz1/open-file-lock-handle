import { expect, type Page } from "@playwright/test";

export type VirtualRowBox = {
  index: number;
  styleTop: number;
  height: number;
  contentHeight: number;
  top: number;
  bottom: number;
};

export type VirtualRowLayout = {
  rows: VirtualRowBox[];
  overlaps: { index: number; pixels: number }[];
  heightMismatches: { index: number; box: number; content: number }[];
};

/** Pure inspector, safe to pass into page.evaluate. */
export function inspectVirtualRows(node: Element): VirtualRowLayout {
  const rows = [...node.querySelectorAll(".data-row")].map((row, index) => {
    const el = row as HTMLElement;
    const box = el.getBoundingClientRect();
    const path = el.querySelector(".path-cell")?.getBoundingClientRect();
    let childBottom = 0;
    for (const child of el.children) {
      const childBox = child.getBoundingClientRect();
      childBottom = Math.max(childBottom, childBox.bottom - box.top);
    }
    const contentHeight = Math.max(
      el.scrollHeight,
      childBottom,
      path ? path.height : 0,
    );
    return {
      index,
      styleTop: Number.parseFloat(el.style.top) || 0,
      height: el.offsetHeight,
      contentHeight,
      top: box.top,
      bottom: box.bottom,
    };
  });
  const overlaps: { index: number; pixels: number }[] = [];
  const heightMismatches: { index: number; box: number; content: number }[] =
    [];
  for (const row of rows) {
    if (row.contentHeight > row.height + 1)
      heightMismatches.push({
        index: row.index,
        box: row.height,
        content: row.contentHeight,
      });
  }
  for (let i = 0; i < rows.length - 1; i++) {
    const byBox = rows[i].bottom - rows[i + 1].top;
    const byStyle = rows[i].styleTop + rows[i].height - rows[i + 1].styleTop;
    const pixels = Math.max(byBox, byStyle);
    if (pixels > 1) overlaps.push({ index: i, pixels });
  }
  return { rows, overlaps, heightMismatches };
}

export function describeVirtualRowLayout(layout: VirtualRowLayout) {
  const parts: string[] = [];
  if (layout.overlaps.length)
    parts.push(
      layout.overlaps
        .map(
          (overlap) =>
            `rows ${overlap.index}/${overlap.index + 1} overlap by ${overlap.pixels.toFixed(1)}px`,
        )
        .join("; "),
    );
  if (layout.heightMismatches.length)
    parts.push(
      layout.heightMismatches
        .map(
          (mismatch) =>
            `row ${mismatch.index} box ${mismatch.box}px < content ${mismatch.content}px`,
        )
        .join("; "),
    );
  return parts.join(" | ") || "layout ok";
}

export const assertVirtualRowsLayout = async (page: Page) => {
  const layout = await page
    .locator("#results-grid")
    .evaluate(inspectVirtualRows);
  expect(layout.overlaps, describeVirtualRowLayout(layout)).toEqual([]);
  expect(layout.heightMismatches, describeVirtualRowLayout(layout)).toEqual([]);
  return layout;
};

export const watchVirtualRowLayout = async (page: Page) => {
  await page.evaluate(
    `window.__inspectVirtualRows = ${inspectVirtualRows.toString()};`,
  );
  await page.locator("#results-grid").evaluate((node) => {
    const inspect = (window as any).__inspectVirtualRows as (
      node: Element,
    ) => VirtualRowLayout;
    const worse = (next: VirtualRowLayout, prev: VirtualRowLayout) => {
      const max = (layout: VirtualRowLayout) =>
        Math.max(0, ...layout.overlaps.map((overlap) => overlap.pixels));
      return (
        next.overlaps.length > prev.overlaps.length ||
        max(next) > max(prev) ||
        next.heightMismatches.length > prev.heightMismatches.length
      );
    };
    const record = () => {
      const layout = inspect(node);
      if (!layout.overlaps.length && !layout.heightMismatches.length) return;
      const prev = (window as any).__virtualRowLayoutViolations as
        VirtualRowLayout | undefined;
      if (!prev || worse(layout, prev))
        (window as any).__virtualRowLayoutViolations = layout;
    };
    (window as any).__virtualRowLayoutViolations = null;
    (window as any).__stopVirtualRowLayoutWatch = false;
    const observer = new MutationObserver(record);
    observer.observe(node, {
      childList: true,
      subtree: true,
      attributes: true,
    });
    const loop = () => {
      record();
      if (!(window as any).__stopVirtualRowLayoutWatch)
        requestAnimationFrame(loop);
      else observer.disconnect();
    };
    loop();
  });
};

export const takeVirtualRowLayoutViolations = async (page: Page) => {
  return page.evaluate(() => {
    (window as any).__stopVirtualRowLayoutWatch = true;
    return (
      ((window as any).__virtualRowLayoutViolations as VirtualRowLayout) ?? {
        rows: [],
        overlaps: [],
        heightMismatches: [],
      }
    );
  });
};

export const assertNoWatchedVirtualRowViolations = async (page: Page) => {
  const layout = await takeVirtualRowLayoutViolations(page);
  expect(layout.overlaps, describeVirtualRowLayout(layout)).toEqual([]);
  expect(layout.heightMismatches, describeVirtualRowLayout(layout)).toEqual([]);
  return layout;
};

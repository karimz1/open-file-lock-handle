import type { Page } from "@playwright/test";

/**
 * Counts how often each named React component renders, through the DevTools
 * global hook (the dev server keeps component names). A component that bails
 * out, for example through React.memo, is not counted. Mounts are counted as
 * renders too; `mounts` lets a test tell updates from first renders.
 */
export async function installRenderProbe(page: Page) {
  await page.addInitScript(() => {
    type Fiber = {
      tag: number;
      type: unknown;
      flags: number;
      alternate: Fiber | null;
      child: Fiber | null;
      sibling: Fiber | null;
    };
    const renders: Record<string, number> = {};
    const mounts: Record<string, number> = {};
    let active = false;
    const nameOf = (fiber: Fiber): string | null => {
      const type = fiber.type as
        | { displayName?: string; name?: string; type?: any; render?: any }
        | null
        | undefined;
      if (!type || typeof type === "string") return null;
      if (typeof type === "function")
        return type.displayName || type.name || null;
      const inner = type.type ?? type.render;
      return inner?.displayName || inner?.name || null;
    };
    const component = new Set([0, 1, 11, 14, 15]);
    const count = (map: Record<string, number>, name: string) => {
      map[name] = (map[name] ?? 0) + 1;
    };
    const walk = (start: Fiber | null) => {
      const stack: Fiber[] = start ? [start] : [];
      while (stack.length) {
        const fiber = stack.pop()!;
        const previous = fiber.alternate;
        if (component.has(fiber.tag)) {
          const name = nameOf(fiber);
          if (name && !previous) {
            count(renders, name);
            count(mounts, name);
          } else if (name && (fiber.flags & 1) === 1) {
            count(renders, name);
          }
        }
        if (fiber.sibling) stack.push(fiber.sibling);
        // A child that was reused as is did not render.
        if (fiber.child && !(previous && previous.child === fiber.child))
          stack.push(fiber.child);
      }
    };
    const hook = {
      supportsFiber: true,
      renderers: new Map(),
      isDisabled: false,
      inject(renderer: unknown) {
        const id = this.renderers.size + 1;
        this.renderers.set(id, renderer);
        return id;
      },
      onCommitFiberRoot(_id: number, root: { current: Fiber }) {
        if (active) walk(root.current.child);
      },
      onCommitFiberUnmount() {},
      onPostCommitFiberRoot() {},
      checkDCE() {},
      on() {},
      off() {},
      emit() {},
      sub() {},
    };
    (window as any).__REACT_DEVTOOLS_GLOBAL_HOOK__ = hook;
    (window as any).__renderProbe = {
      begin() {
        for (const key of Object.keys(renders)) delete renders[key];
        for (const key of Object.keys(mounts)) delete mounts[key];
        active = true;
      },
      end() {
        active = false;
        return { renders: { ...renders }, mounts: { ...mounts } };
      },
    };
  });
}

export type RenderCounts = {
  renders: Record<string, number>;
  mounts: Record<string, number>;
};

export async function countRenders(
  page: Page,
  action: () => Promise<void>,
): Promise<RenderCounts> {
  await page.evaluate(() => (window as any).__renderProbe.begin());
  await action();
  // Let debounced work and effects settle so late renders are counted.
  await page.waitForTimeout(400);
  return page.evaluate(() => (window as any).__renderProbe.end());
}

/** Renders that were updates of an already mounted component. */
export function updates(counts: RenderCounts, name: string) {
  return (counts.renders[name] ?? 0) - (counts.mounts[name] ?? 0);
}

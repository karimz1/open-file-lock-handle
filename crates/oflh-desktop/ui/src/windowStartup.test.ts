import { describe, expect, it, vi } from "vitest";
import {
  OPEN_MAXIMIZED_KEY,
  applyStartupWindowState,
  readOpenMaximized,
  writeOpenMaximized,
} from "./windowStartup";

const memoryStorage = (initial: Record<string, string> = {}) => {
  const values = new Map(Object.entries(initial));
  return {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => void values.set(key, value),
  };
};
const brokenStorage = {
  getItem: () => {
    throw new Error("storage disabled");
  },
  setItem: () => {
    throw new Error("storage disabled");
  },
};

describe("open window maximized setting", () => {
  it("is off by default and for unknown or unreadable values", () => {
    expect(readOpenMaximized(memoryStorage())).toBe(false);
    expect(
      readOpenMaximized(memoryStorage({ [OPEN_MAXIMIZED_KEY]: "true" })),
    ).toBe(false);
    expect(readOpenMaximized(brokenStorage)).toBe(false);
  });
  it("round-trips through storage", () => {
    const storage = memoryStorage();
    writeOpenMaximized(storage, true);
    expect(storage.getItem(OPEN_MAXIMIZED_KEY)).toBe("1");
    expect(readOpenMaximized(storage)).toBe(true);
    writeOpenMaximized(storage, false);
    expect(readOpenMaximized(storage)).toBe(false);
    expect(() => writeOpenMaximized(brokenStorage, true)).not.toThrow();
  });
  it("maximizes at startup only when the setting is on", async () => {
    const maximize = vi.fn(async () => {});
    expect(await applyStartupWindowState(memoryStorage(), maximize)).toBe(
      false,
    );
    expect(maximize).not.toHaveBeenCalled();
    const on = memoryStorage({ [OPEN_MAXIMIZED_KEY]: "1" });
    expect(await applyStartupWindowState(on, maximize)).toBe(true);
    expect(maximize).toHaveBeenCalledTimes(1);
  });
  it("keeps the restore size when maximizing fails", async () => {
    const on = memoryStorage({ [OPEN_MAXIMIZED_KEY]: "1" });
    const failing = vi.fn(async () => {
      throw new Error("not allowed");
    });
    expect(await applyStartupWindowState(on, failing)).toBe(false);
  });
});

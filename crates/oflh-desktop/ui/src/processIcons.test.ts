import { describe, expect, it, vi } from "vitest";
import { createIconCache } from "./processIcons";

describe("process icon cache", () => {
  it("requests each executable once and shares the result", async () => {
    const load = vi.fn(async () => "data:image/png;base64,AQID");
    const cache = createIconCache(load);
    const [a, b] = await Promise.all([
      cache.get(1, "1:0:exe", "4000:1:0"),
      cache.get(1, "1:0:exe", "4000:1:0"),
    ]);
    expect(a).toBe("data:image/png;base64,AQID");
    expect(b).toBe(a);
    expect(load).toHaveBeenCalledTimes(1);
    expect(cache.peek("1:0:exe")).toBe(a);
    expect(cache.forProcess("4000:1:0")).toBe(a);
  });

  it("falls back to the neutral icon on missing icons and errors without retrying", async () => {
    const load = vi
      .fn()
      .mockResolvedValueOnce(null)
      .mockRejectedValueOnce(new Error("no icon"))
      .mockResolvedValueOnce("");
    const cache = createIconCache(load);
    expect(await cache.get(1, "1:0:exe")).toBeNull();
    expect(await cache.get(1, "1:1:exe")).toBeNull();
    expect(await cache.get(1, "1:2:exe")).toBeNull();
    expect(await cache.get(1, "1:1:exe")).toBeNull();
    expect(load).toHaveBeenCalledTimes(3);
    expect(cache.peek("1:1:exe")).toBeNull();
    expect(cache.peek("1:9:exe")).toBeUndefined();
  });

  it("never asks for rows without an executable reference", async () => {
    const load = vi.fn(async () => "x");
    const cache = createIconCache(load);
    expect(await cache.get(1, "")).toBeNull();
    expect(load).not.toHaveBeenCalled();
  });

  it("notifies the confirmation when a process icon arrives", async () => {
    const cache = createIconCache(async () => "data:image/png;base64,AA==");
    const listener = vi.fn();
    const stop = cache.subscribe(listener);
    await cache.get(1, "1:0:exe", "42:1:0");
    expect(listener).toHaveBeenCalledTimes(1);
    stop();
    await cache.get(1, "1:1:exe", "43:1:0");
    expect(listener).toHaveBeenCalledTimes(1);
    expect(cache.forProcess("99:1:0")).toBeNull();
  });

  it("stays bounded", async () => {
    const load = vi.fn(async (_: number, reference: string) => reference);
    const cache = createIconCache(load, 2);
    for (const reference of ["a", "b", "c"]) await cache.get(1, reference);
    expect(cache.peek("a")).toBeUndefined();
    expect(cache.peek("c")).toBe("c");
  });
});

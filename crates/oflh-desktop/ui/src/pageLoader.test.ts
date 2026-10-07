import { expect, it, vi } from "vitest";
import { createPageLoader } from "./pageLoader";
import type { Page, TableQuery } from "./api";
const query = { text: "", offset: 0, limit: 200 } as TableQuery;
const flush = () => new Promise<void>((resolve) => setTimeout(resolve, 0));
it("bounds IPC work and replaces obsolete pending searches with the latest request", async () => {
  const resolvers: ((page: Page) => void)[] = [];
  const transport = vi.fn(
    () => new Promise<Page>((resolve) => resolvers.push(resolve)),
  );
  const accept = vi.fn();
  const reject = vi.fn();
  const loader = createPageLoader(transport);
  loader.request(1, { ...query, text: "first" }, accept, reject);
  loader.request(1, { ...query, text: "second" }, accept, reject);
  loader.request(2, { ...query, text: "latest" }, accept, reject);
  expect(transport).toHaveBeenCalledTimes(1);
  resolvers[0]({ revision: 1, total: 0, rows: [] });
  await flush();
  expect(accept).not.toHaveBeenCalled();
  expect(transport).toHaveBeenCalledTimes(2);
  expect(transport.mock.calls[1]).toEqual([2, { ...query, text: "latest" }]);
  resolvers[1]({ revision: 2, total: 0, rows: [] });
  await flush();
  expect(accept).toHaveBeenCalledOnce();
  expect(reject).not.toHaveBeenCalled();
});
it("cancellation suppresses stale errors and drops queued work", async () => {
  let rejectTransport: (error: unknown) => void = () => {};
  const transport = vi.fn(
    () =>
      new Promise<Page>((_, reject) => {
        rejectTransport = reject;
      }),
  );
  const accept = vi.fn();
  const reject = vi.fn();
  const loader = createPageLoader(transport);
  loader.request(1, query, accept, reject);
  loader.request(1, { ...query, text: "queued" }, accept, reject);
  loader.cancel();
  rejectTransport(new Error("obsolete"));
  await flush();
  expect(transport).toHaveBeenCalledTimes(1);
  expect(reject).not.toHaveBeenCalled();
  loader.request(2, query, accept, reject);
  rejectTransport(new Error("current"));
  await flush();
  expect(reject).toHaveBeenCalledOnce();
});
it("rejects pages from a different snapshot revision", async () => {
  const accept = vi.fn();
  const loader = createPageLoader(async () => ({
    revision: 1,
    total: 0,
    rows: [],
  }));
  loader.request(2, query, accept, vi.fn());
  await flush();
  expect(accept).not.toHaveBeenCalled();
});

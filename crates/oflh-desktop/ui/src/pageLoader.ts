import { api, type Page, type TableQuery } from "./api";

// Keep one IPC query running and coalesce one pending request. Native work already
// dispatched may finish; obsolete callbacks must never update rows or errors.
export function createPageLoader(transport = api.page) {
  type Request = {
    identity: number;
    revision: number;
    query: TableQuery;
    accept: (page: Page) => void;
    reject: (error: unknown) => void;
  };
  let identity = 0;
  let running = false;
  let pending: Request | null = null;
  const pump = () => {
    if (running || !pending) return;
    const request = pending;
    pending = null;
    running = true;
    void transport(request.revision, request.query)
      .then((page) => {
        if (request.identity === identity && page.revision === request.revision)
          request.accept(page);
      })
      .catch((error) => {
        if (request.identity === identity) request.reject(error);
      })
      .finally(() => {
        running = false;
        pump();
      });
  };
  return {
    request(
      revision: number,
      query: TableQuery,
      accept: (page: Page) => void,
      reject: (error: unknown) => void,
    ) {
      pending = { identity: ++identity, revision, query, accept, reject };
      pump();
    },
    cancel() {
      identity++;
      pending = null;
    },
  };
}

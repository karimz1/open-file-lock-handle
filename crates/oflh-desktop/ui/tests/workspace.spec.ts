import { test, expect } from "@playwright/test";
// Synthetic IPC fixtures are test-only. The production bundle always invokes Rust.
const documentationScreenshotPath = (filename: string) =>
  process.env.OFLH_UPDATE_SCREENSHOTS === "1"
    ? `../../../images/${filename}`
    : `test-results/${filename}`;

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    if (!sessionStorage.getItem("theme-test-initialized")) {
      localStorage.setItem("oflh-theme", "light");
      sessionStorage.setItem("theme-test-initialized", "1");
    }
  });
  await page.addInitScript(() => {
    const calls: { command: string; args: Record<string, unknown> }[] = [];
    const callbacks = new Map<number, (payload: unknown) => void>();
    const callbackEvents = new Map<number, string>();
    let callbackId = 0;
    let terminated = false;
    const processRows = Array.from({ length: 1500 }, (_, index) => ({
      key: `${4000 + index}:18446744073709551615:0/exe`,
      process_key: `${4000 + index}:18446744073709551615:0`,
      name: ["Code", "node", "rust-analyzer", "cargo", "FixtureWorker"][
        index % 5
      ],
      pid: 4000 + index,
      user: "developer",
      path: "/workspace/project/target/debug/fixture",
      path_ref: `1:${index}:exe`,
      relation: "process",
      access: "unknown",
      evidence: null,
      evidence_label: null,
      deleted: false,
      memory: 73400320,
      cpu: null,
      usages: 12,
      actionable: true,
      port: null,
    }));
    const portRows = [
      {
        ...processRows[0],
        key: "4000:18446744073709551615:0/TCP-127.0.0.1:8080",
        port: {
          protocol: "TCP",
          state: "LISTEN",
          address: "127.0.0.1",
          number: 8080,
          endpoint: "127.0.0.1:8080",
          reference: "1:0:port-0",
        },
      },
      {
        ...processRows[1],
        key: "0:0:0/UDP-[::]:5353",
        name: "owner unavailable",
        path: "",
        memory: null,
        cpu: null,
        pid: 0,
        process_key: "0:0:0",
        actionable: false,
        port: {
          protocol: "UDP",
          state: "BOUND",
          address: "::",
          number: 5353,
          endpoint: "[::]:5353",
          reference: "1:1:port-0",
        },
      },
    ];
    let status = {
      generation: 1,
      revision: 1,
      scanning: false,
      target: "/workspace/project",
      processes: 1500,
      ports: 2,
      usages: 18000,
      warnings: [
        "Some processes could not be inspected because access was denied.",
      ],
      error: null,
      version:
        new URL(location.href).searchParams.get("version") ?? "development",
      commit: "0123456789abcdef0123456789abcdef01234567",
      build_url:
        "https://github.com/karimz1/open-file-lock-handle/actions/runs/1234567890",
      pull_request_url:
        "https://github.com/karimz1/open-file-lock-handle/pull/42",
    };
    if (new URL(location.href).searchParams.has("empty-target")) {
      status = {
        ...status,
        revision: 0,
        target: "",
        processes: 0,
        ports: 0,
        usages: 0,
      };
    }
    let recentTargets = [
      { id: 1, display: "/workspace/project" },
      { id: 2, display: "/workspace/another-project" },
      { id: 3, display: "/tmp/fixture" },
      ...Array.from({ length: 9 }, (_, index) => ({
        id: index + 4,
        display: `/workspace/generated-${index}`,
      })),
    ];
    Object.assign(window, {
      __testCalls: calls,
      __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener: () => {} },
      __TAURI_INTERNALS__: {
        transformCallback(callback: (payload: unknown) => void) {
          callbacks.set(++callbackId, callback);
          return callbackId;
        },
        unregisterCallback(id: number) {
          callbacks.delete(id);
          callbackEvents.delete(id);
        },
        async invoke(command: string, args: Record<string, any> = {}) {
          calls.push({ command, args });
          if (command === "plugin:event|listen") {
            callbackEvents.set(args.handler, args.event);
            return 1;
          }
          if (command === "plugin:event|unlisten") return;
          if (command === "update_mode")
            return (window as any).__updateMode ?? "install";
          if (command === "plugin:resources|close") return;
          if (command === "plugin:updater|check") {
            if ((window as any).__updateCheckFails)
              throw new Error("Synthetic update endpoint failure");
            return (window as any).__updateMetadata ?? null;
          }
          if (command === "plugin:updater|download_and_install") {
            if ((window as any).__updateInstallFails)
              throw new Error("Synthetic update signature failure");
            return;
          }
          if (command === "plugin:process|restart") return;
          if (command === "status") return status;
          if (command === "recent") return recentTargets;
          if (command === "remove_recent") {
            recentTargets = recentTargets.filter(
              (target) => target.id !== args.id,
            );
            return;
          }
          if (command === "clear_recent") {
            recentTargets = [];
            return;
          }
          if (command === "refresh" && (window as any).__holdRefreshForTest) {
            return (status = {
              ...status,
              generation: status.generation + 1,
              scanning: true,
            });
          }
          if (command === "reveal" && (window as any).__failReveal) {
            throw {
              kind: "desktop_integration",
              message:
                "Could not reveal target; the fallback action also failed",
              os_code: null,
              details:
                "Primary action failed: FileManager1 is unavailable\nFallback action failed: xdg-open failed\nRust backtrace: fixture stack",
            };
          }
          if (["refresh", "inspect", "revisit", "choose"].includes(command))
            return (status = {
              ...status,
              generation: status.generation + 1,
              revision:
                terminated || (window as any).__advanceRevisionForTest
                  ? status.revision + 1
                  : status.revision,
            });
          if (command === "page" && args.query.ports) {
            const rows = portRows.filter(
              (row) =>
                (!args.query.text ||
                  row.port.number.toString() ===
                    args.query.text.replace("port:", "")) &&
                (!args.query.ports_path_only || row.pid > 0) &&
                (!args.query.process_key ||
                  row.process_key === args.query.process_key) &&
                !(terminated && row.pid === 4000),
            );
            return { revision: status.revision, total: rows.length, rows };
          }
          if (command === "page") {
            const rows = processRows.filter(
              (row) =>
                (!args.query.text ||
                  row.name
                    .toLowerCase()
                    .includes(args.query.text.toLowerCase())) &&
                (!args.query.columns?.name ||
                  row.name
                    .toLowerCase()
                    .includes(args.query.columns.name.toLowerCase())),
            );
            return {
              revision: 1,
              total: rows.length,
              rows: rows.slice(
                args.query.offset,
                args.query.offset + args.query.limit,
              ),
            };
          }
          if (command === "details") {
            const process = [...processRows, ...portRows].find(
              (row) => row.process_key === args.key,
            );
            return {
              process,
              ports: 1,
              can_inspect_folder: process?.pid !== 0,
              executable: {
                display: process?.path,
                reference: process?.path_ref,
              },
              cwd: { display: "/workspace/project", reference: "1:0:cwd" },
              ancestors: [
                {
                  name: "fixture-shell",
                  pid: 3000,
                  key: "3000:900:0",
                  actionable: true,
                },
                {
                  name: "fixture-init",
                  pid: 1,
                  key: "1:1:0",
                  actionable: false,
                },
              ],
            };
          }
          if (command === "select_all")
            return processRows
              .filter(
                (row) =>
                  !args.query.text ||
                  row.name
                    .toLowerCase()
                    .includes(args.query.text.toLowerCase()),
              )
              .map((row) => row.process_key);
          if (command === "prepare")
            return {
              ticket: "captured-ticket",
              force: args.force,
              targets: args.keys.map((key: string) => {
                const row = processRows.find((row) => row.process_key === key)!;
                return { key, name: row.name, pid: row.pid };
              }),
            };
          if (command === "prepare_elevated")
            return {
              ticket: "admin-ticket",
              force: (window as any).__adminForce ?? true,
              elevated: true,
              targets: [
                {
                  key: "4000:18446744073709551615:0",
                  name: "fixture-process",
                  pid: 4000,
                },
              ],
            };
          if (command === "prepare_ancestor")
            return {
              ticket: "parent-ticket",
              force: args.force,
              targets: [{ key: args.key, name: "fixture-shell", pid: 3000 }],
            };
          if (command === "terminate" && (window as any).__terminationResults) {
            return (window as any).__terminationResults;
          }
          if (command === "terminate" && args.ticket === "parent-ticket") {
            await new Promise((resolve) => setTimeout(resolve, 200));
            return [{ pid: 3000, outcome: "exited", error: null }];
          }
          if (
            command === "terminate" &&
            (window as any).__terminateSuccessfully
          ) {
            terminated = true;
            return [{ pid: 4000, outcome: "exited", error: null }];
          }
          if (command === "terminate")
            return [
              {
                pid: 4000,
                outcome: "failed",
                error: {
                  kind: "identity_changed",
                  message:
                    "Process exited or PID was reused; refresh before trying again",
                  os_code: null,
                },
              },
            ];
          if (
            [
              "dismiss",
              "copy",
              "copy_diagnostic",
              "reveal",
              "open_project",
              "open_profile",
              "open_donation",
              "open_sponsors",
              "open_release_notes",
              "open_download",
              "open_issue",
            ].includes(command)
          )
            return;
          throw new Error(`Unexpected test IPC command ${command}`);
        },
      },
      __emitTestEvent(event: string, payload: unknown) {
        for (const [id, callback] of callbacks) {
          if (callbackEvents.get(id) === event) {
            callback({ event, id, payload });
          }
        }
      },
    });
  });
});
test("virtualized workspace, theme, process details, keyboard and copy", async ({
  page,
}) => {
  await page.goto("/");
  const grid = page.getByRole("grid");
  await expect(grid.getByText("4000", { exact: true })).toBeVisible();
  expect(await grid.getByRole("row").count()).toBeLessThan(60);
  await expect(page.getByText("1500 results")).toBeVisible();
  const first = grid.getByRole("row").nth(1);
  await first.dblclick();
  await expect(
    page.getByRole("complementary", { name: "Process details" }),
  ).toBeVisible();
  await expect(page.getByText("1 process selected")).toBeVisible();
  await page.keyboard.press("Control+c");
  await expect(
    page.getByRole("status").filter({ hasText: "Selection copied" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Close process details" }).click();
  await page.screenshot({ path: "test-results/workspace-light.png" });
  await page.keyboard.press("Control+,");
  await page.getByRole("button", { name: "VS Code Dark", exact: true }).click();
  await page.getByRole("button", { name: /^Processes/ }).click();
  await expect(grid.getByText("4000", { exact: true })).toBeVisible();
  await page.screenshot({ path: "test-results/workspace-dark.png" });
  await grid.focus();
  await page.keyboard.press("End");
  await expect(grid.getByText("5499", { exact: true })).toBeVisible();
  expect(await grid.getByRole("row").count()).toBeLessThan(60);
});
test("whole sortable headers work and optional columns persist", async ({
  page,
}) => {
  await page.goto("/");
  const grid = page.getByRole("grid");
  const pidHeader = grid
    .getByRole("columnheader")
    .filter({ has: page.getByRole("button", { name: "Sort by PID" }) });
  const sortButton = pidHeader.getByRole("button");
  const sortBounds = await sortButton.boundingBox();
  await sortButton.click({
    position: { x: sortBounds!.width - 20, y: sortBounds!.height / 2 },
  });
  await expect
    .poll(async () => {
      const calls = await page.evaluate(() => (window as any).__testCalls);
      return calls.filter((call: any) => call.command === "page").at(-1).args
        .query.sort;
    })
    .toBe("pid");

  await page.getByText("Columns", { exact: true }).click();
  const columns = page.getByRole("group", { name: "Visible columns" });
  await columns.getByLabel("CPU", { exact: true }).uncheck();
  await expect(grid.getByRole("columnheader", { name: "CPU" })).toHaveCount(0);
  await page.reload();
  await expect(
    page.getByRole("grid").getByRole("columnheader", { name: "CPU" }),
  ).toHaveCount(0);
  await page.getByText("Columns", { exact: true }).click();
  await page
    .getByRole("group", { name: "Visible columns" })
    .getByLabel("CPU", { exact: true })
    .check();
  await expect(
    page.getByRole("grid").getByRole("columnheader", { name: "CPU" }),
  ).toBeVisible();
});
test("dark themes keep UI text and surface boundaries distinct", async ({
  page,
}) => {
  await page.goto("/");
  await page.keyboard.press("Control+,");
  const contrast = (foreground: string, background: string) => {
    const luminance = (color: string) => {
      const channels = color
        .match(/[\da-f]{2}/gi)!
        .map((channel) => parseInt(channel, 16) / 255)
        .map((channel) =>
          channel <= 0.04045
            ? channel / 12.92
            : ((channel + 0.055) / 1.055) ** 2.4,
        );
      return channels[0] * 0.2126 + channels[1] * 0.7152 + channels[2] * 0.0722;
    };
    const values = [luminance(foreground), luminance(background)].sort(
      (left, right) => right - left,
    );
    return (values[0] + 0.05) / (values[1] + 0.05);
  };
  for (const theme of ["Rider Dark", "VS Code Dark"]) {
    await page.getByRole("button", { name: theme, exact: true }).click();
    const colors = await page.locator("html").evaluate((element) => {
      const style = getComputedStyle(element);
      return Object.fromEntries(
        ["bg", "panel", "sidebar", "text", "muted", "border"].map((name) => [
          name,
          style.getPropertyValue(`--${name}`).trim(),
        ]),
      ) as Record<string, string>;
    });
    expect(contrast(colors.text, colors.panel)).toBeGreaterThan(10);
    expect(contrast(colors.muted, colors.panel)).toBeGreaterThan(7);
    expect(contrast(colors.border, colors.panel)).toBeGreaterThan(2.5);
    expect(contrast(colors.panel, colors.bg)).toBeGreaterThan(1.1);
    expect(contrast(colors.sidebar, colors.panel)).toBeGreaterThan(1.1);
  }
});
test("file usage cells show the complete path", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: /^File usages/ }).click();
  const firstRow = page.getByRole("grid").getByRole("row").nth(1);
  await expect(firstRow.getByRole("gridcell").nth(2)).toHaveText(
    "/workspace/project/target/debug/fixture",
  );
  await expect(firstRow.getByRole("gridcell").nth(2)).toHaveCSS(
    "white-space",
    "normal",
  );
});
test("reveal errors expose diagnostics and a reproducible issue draft", async ({
  page,
}) => {
  await page.goto("/");
  await page.evaluate(() => {
    (window as any).__failReveal = true;
  });
  const firstRow = page.getByRole("grid").getByRole("row").nth(1);
  const reportCalls: { command: string; args: Record<string, any> }[] = [];

  for (const action of ["Reveal in file manager", "Open containing folder"]) {
    await firstRow.click({ button: "right" });
    await page.getByRole("button", { name: action, exact: true }).click();
    const errorBanner = page.getByRole("alert");
    await expect(errorBanner).toContainText("Operation could not complete");
    await expect(errorBanner).toContainText(action);
    await errorBanner.getByRole("button", { name: "Details" }).click();
    const detailsDialog = page.getByRole("dialog", {
      name: "Operation details",
    });
    const details = detailsDialog.locator(".error-details-modal");
    await expect(details).toContainText("FileManager1 is unavailable");
    await expect(details).toContainText("xdg-open failed");
    await expect(details).toContainText("Rust backtrace: fixture stack");
    await detailsDialog.getByRole("button", { name: "Copy details" }).click();
    await expect(page.getByText("Error details copied")).toBeVisible();
    await detailsDialog
      .getByRole("button", { name: "Close", exact: true })
      .click();
    await errorBanner.getByRole("button", { name: "Open issue" }).click();
    const calls = await page.evaluate(() => (window as any).__testCalls);
    expect(
      calls.find((call: any) => call.command === "copy_diagnostic").args.text,
    ).toContain("Rust backtrace: fixture stack");
    reportCalls.push(
      calls.filter((call: any) => call.command === "open_issue").at(-1),
    );
    await errorBanner.getByRole("button", { name: "Dismiss error" }).click();
  }

  const revealCalls = await page.evaluate(() =>
    (window as any).__testCalls.filter(
      (call: any) => call.command === "reveal",
    ),
  );
  expect(revealCalls.map((call: any) => call.args.containing)).toEqual([
    false,
    true,
  ]);
  expect(reportCalls).toHaveLength(2);
  for (const call of reportCalls) {
    expect(call.args.body).toContain("Steps to reproduce");
    expect(call.args.body).toContain("OFLH version: development");
    expect(call.args.body).toContain("Rust backtrace: fixture stack");
    expect(call.args.body).toContain("remove any private paths");
  }
  expect(reportCalls[0].args.body).toContain("Choose Reveal in file manager");
  expect(reportCalls[1].args.body).toContain("Choose Open containing folder");
});
test("auto refresh pauses while context actions are open", async ({ page }) => {
  await page.clock.install();
  await page.goto("/");
  await page.getByLabel("Automatic refresh interval").selectOption("5");
  const row = page.getByRole("grid").getByRole("row").nth(1);
  await row.click({ button: "right" });
  await expect(page.getByRole("dialog")).toBeVisible();
  await page.evaluate(() => {
    (window as any).__advanceRevisionForTest = true;
  });
  await page.clock.fastForward(5000);
  await expect(page.getByRole("dialog")).toBeVisible();
  let refreshCalls = await page.evaluate(
    () =>
      (window as any).__testCalls.filter(
        (call: any) => call.command === "refresh",
      ).length,
  );
  expect(refreshCalls).toBe(0);
  await page
    .getByRole("button", { name: "Reveal in file manager", exact: true })
    .click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await page.clock.fastForward(5000);
  await expect
    .poll(async () => {
      refreshCalls = await page.evaluate(
        () =>
          (window as any).__testCalls.filter(
            (call: any) => call.command === "refresh",
          ).length,
      );
      return refreshCalls;
    })
    .toBeGreaterThan(0);
  await expect(page.getByText("Operation could not complete")).toHaveCount(0);
  const revealCalls = await page.evaluate(() =>
    (window as any).__testCalls.filter(
      (call: any) => call.command === "reveal",
    ),
  );
  expect(revealCalls).toHaveLength(1);
});
test("recent targets can be searched, removed, and cleared", async ({
  page,
}) => {
  await page.goto("/");
  await page
    .getByRole("navigation")
    .getByRole("button", { name: /Recent targets/ })
    .click();
  const recentList = page.locator('.recent-list[aria-label="Recent targets"]');
  await expect(recentList.locator(".recent-entry")).toHaveCount(12);
  const dimensions = await recentList.evaluate((element: HTMLElement) => ({
    clientHeight: element.clientHeight,
    scrollHeight: element.scrollHeight,
  }));
  expect(dimensions.scrollHeight).toBeGreaterThan(dimensions.clientHeight);

  const search = page.getByRole("searchbox", { name: "Search recent targets" });
  await search.fill("another-project");
  await expect(page.getByText("1 of 12", { exact: true })).toBeVisible();
  await expect(
    recentList.getByRole("button", {
      name: "/workspace/another-project",
      exact: true,
    }),
  ).toBeVisible();

  await search.fill("");
  await recentList
    .getByRole("button", { name: "Remove /workspace/another-project" })
    .click();
  await expect(recentList.locator(".recent-entry")).toHaveCount(11);
  await page.getByRole("button", { name: "Clear all", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "No recent targets" }),
  ).toBeVisible();
});
test("automatic refresh repeats at the selected interval", async ({ page }) => {
  await page.goto("/");
  await page.getByLabel("Automatic refresh interval").selectOption("5");
  await expect
    .poll(
      async () => {
        const calls = await page.evaluate(() => (window as any).__testCalls);
        return calls.filter((call: any) => call.command === "refresh").length;
      },
      { timeout: 7000 },
    )
    .toBeGreaterThan(0);
});
test("automatic refresh is session-only and waits for a completed scan", async ({
  page,
}) => {
  await page.goto("/?empty-target");
  await expect(page.getByLabel("Automatic refresh interval")).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "Refresh", exact: true }),
  ).toBeDisabled();

  await page
    .getByRole("textbox", { name: "Target file or folder path" })
    .fill("/workspace/project");
  await page.evaluate(() => {
    (window as any).__advanceRevisionForTest = true;
  });
  await page.getByRole("button", { name: "Inspect", exact: true }).click();
  const interval = page.getByLabel("Automatic refresh interval");
  await expect(interval).toBeEnabled();
  await page
    .locator('summary[aria-label="Automatic refresh information"]')
    .click();
  await expect(page.getByRole("note")).toContainText("Off by default");
  await expect(page.getByRole("note")).toContainText(
    "manual refresh is often better for a focused check",
  );
  await interval.selectOption("5");
  await expect
    .poll(() =>
      page.evaluate(() => localStorage.getItem("oflh-auto-reload-seconds")),
    )
    .toBeNull();

  await page.reload();
  await expect(page.getByLabel("Automatic refresh interval")).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "Refresh", exact: true }),
  ).toBeDisabled();
});
test("scan progress does not move the results grid", async ({ page }) => {
  await page.goto("/");
  const grid = page.getByRole("grid");
  const before = await grid.boundingBox();
  await page.evaluate(() => {
    (window as any).__holdRefreshForTest = true;
  });
  await page.getByRole("button", { name: "Refresh", exact: true }).click();
  await expect(
    page.getByRole("status").filter({ hasText: "Scanning" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Cancel", exact: true }),
  ).toBeVisible();
  const during = await grid.boundingBox();
  expect(during!.y).toBe(before!.y);
  expect(during!.height).toBe(before!.height);
});
test("hidden selection confirmation defaults to cancel and preserves force mode", async ({
  page,
}) => {
  await page.goto("/");
  const grid = page.getByRole("grid");
  await expect(page.getByText("1500 results")).toBeVisible();
  await grid.getByRole("row").nth(1).click();
  await page
    .getByRole("textbox", { name: "Search loaded results" })
    .fill("node");
  await expect(page.getByText("300 results")).toBeVisible();
  await page
    .getByRole("button", { name: "Force terminate…", exact: true })
    .click();
  const dialog = page.getByRole("dialog");
  await expect(
    dialog.getByRole("button", { name: "Cancel", exact: true }),
  ).toBeFocused();
  await expect(dialog.getByText("PID 4000")).toBeVisible();
  await page.screenshot({ path: "test-results/confirmation.png" });
  await page.keyboard.press("Escape");
  await expect(dialog).not.toBeVisible();
  await page.getByRole("button", { name: "Terminate…", exact: true }).click();
  await dialog.getByRole("button", { name: "Terminate", exact: true }).click();
  await expect(
    page.getByText(
      "Process changed or already exited. No termination was sent.",
    ),
  ).toBeVisible();
  const calls = await page.evaluate(
    () =>
      (
        window as unknown as {
          __testCalls: { command: string; args: Record<string, unknown> }[];
        }
      ).__testCalls,
  );
  expect(calls.find((call) => call.command === "terminate")?.args).toEqual({
    ticket: "captured-ticket",
  });
  expect(
    calls
      .filter((call) => call.command === "prepare")
      .map((call) => call.args.force),
  ).toEqual([true, false]);
});

test("port searches stay in Rust IPC and expose binding actions safely", async ({
  page,
}) => {
  await page.goto("/");
  await expect(page.getByText("1500 results")).toBeVisible();
  await page.getByRole("button", { name: /^Ports/ }).click();
  const grid = page.getByRole("grid", {
    name: "Local TCP listeners and UDP bindings",
  });
  await expect(grid.getByText("8080", { exact: true })).toBeVisible();
  await expect(grid.getByText("BOUND", { exact: true })).toBeVisible();
  await page.screenshot({ path: "test-results/ports.png" });
  await grid
    .getByText("owner unavailable", { exact: true })
    .click({ button: "right" });
  await expect(
    page.getByRole("dialog").getByRole("button", { name: "Force terminate…" }),
  ).toHaveCount(0);
  await page.keyboard.press("Escape");
  await page
    .getByRole("textbox", { name: "Search loaded results" })
    .fill("port:8080");
  await expect(page.getByText("1 results", { exact: true })).toBeVisible();
  await grid.getByText("8080", { exact: true }).click({ button: "right" });
  await page
    .getByRole("button", { name: "Copy local endpoint", exact: true })
    .click();
  const calls = await page.evaluate(
    () =>
      (
        window as unknown as {
          __testCalls: { command: string; args: Record<string, any> }[];
        }
      ).__testCalls,
  );
  expect(
    calls.some(
      (call) =>
        call.command === "page" &&
        call.args.query.ports &&
        call.args.query.text === "port:8080",
    ),
  ).toBe(true);
  expect(calls.find((call) => call.command === "copy")?.args.reference).toBe(
    "1:0:port-0",
  );
});

test("parent termination verifies exit, clears stale details and refreshes", async ({
  page,
}) => {
  await page.goto("/");
  await expect(page.getByText("1500 results")).toBeVisible();
  await page.getByRole("grid").getByRole("row").nth(1).dblclick();
  await page.getByRole("button", { name: "fixture-shell 3000" }).click();
  await page
    .getByRole("button", { name: "Terminate parent…", exact: true })
    .click();
  const dialog = page.getByRole("dialog");
  await expect(
    dialog.getByRole("button", { name: "Cancel", exact: true }),
  ).toBeFocused();
  await dialog.getByRole("button", { name: "Terminate", exact: true }).click();
  await expect(page.getByText("Waiting for process exit…")).toBeVisible();
  await expect(page.getByText("Process exited", { exact: true })).toBeVisible();
  await expect(
    page.getByRole("complementary", { name: "Process details" }),
  ).not.toBeVisible();
  await expect(
    page.getByText("Operation could not complete"),
  ).not.toBeVisible();
  await page.getByRole("button", { name: "Done", exact: true }).click();
  await page.getByRole("button", { name: "Star on GitHub" }).click();
  const calls = await page.evaluate(() => (window as any).__testCalls);
  expect(calls.some((call: any) => call.command === "refresh")).toBe(true);
  expect(
    calls.find((call: any) => call.command === "prepare_ancestor").args,
  ).toEqual({
    owner: "4000:18446744073709551615:0",
    key: "3000:900:0",
    force: false,
  });
  expect(calls.some((call: any) => call.command === "open_project")).toBe(true);
});

test("details splitter and font preferences resize and persist", async ({
  page,
}) => {
  await page.goto("/");
  await expect(page.getByText("1500 results")).toBeVisible();
  await page.getByRole("grid").getByRole("row").nth(1).dblclick();
  const splitter = page.getByRole("separator", {
    name: "Resize process details",
  });
  await expect(splitter).toHaveAttribute("aria-valuenow", "340");
  await splitter.focus();
  await page.keyboard.press("ArrowLeft");
  await expect(splitter).toHaveAttribute("aria-valuenow", "360");
  const box = (await splitter.boundingBox())!;
  await page.mouse.move(box.x + 4, box.y + 40);
  await page.mouse.down();
  await page.mouse.move(box.x - 56, box.y + 40);
  await page.mouse.up();
  await expect(splitter).toHaveAttribute("aria-valuenow", "420");
  await page.keyboard.press("Control+,");
  await page
    .getByRole("combobox", { name: "Size", exact: true })
    .selectOption("18");
  await expect(page.locator("html")).toHaveCSS("font-size", "18px");
  await page.reload();
  await expect(page.locator("html")).toHaveCSS("font-size", "18px");
  await page.getByRole("grid").getByRole("row").nth(1).dblclick();
  await expect(splitter).toHaveAttribute("aria-valuenow", "420");
  await expect(page.getByRole("grid").getByRole("row").nth(1)).toHaveCSS(
    "height",
    "53px",
  );
  await page.screenshot({ path: "test-results/large-font-details.png" });
  await page.setViewportSize({ width: 860, height: 700 });
  const panel = await page
    .getByRole("complementary", { name: "Process details" })
    .boundingBox();
  expect(panel!.x + panel!.width).toBeLessThanOrEqual(861);
});

test("theme presets persist and ancestry reads from parent to highlighted process", async ({
  page,
}) => {
  await page.goto("/");
  for (const [label, id] of [
    ["Rider Dark", "rider"],
    ["VS Code Dark", "vscode"],
    ["OFLH Purple", "purple"],
  ]) {
    await page.keyboard.press("Control+,");
    const choice = page.getByRole("button", { name: label, exact: true });
    await choice.click();
    await expect(choice).toHaveAttribute("aria-pressed", "true");
    await expect(page.locator("html")).toHaveAttribute("data-theme", id);
    await page.screenshot({ path: `test-results/theme-${id}.png` });
    await page.reload();
    await expect(page.locator("html")).toHaveAttribute("data-theme", id);
  }
  await page.getByRole("grid").getByRole("row").nth(1).dblclick();
  const ancestry = page.getByRole("list", { name: "Process ancestry" });
  await expect(ancestry.getByRole("button").nth(0)).toHaveText(/fixture-init/);
  await expect(ancestry.getByRole("button").nth(1)).toHaveText(/fixture-shell/);
  const current = ancestry.getByRole("button", {
    name: /Code Current process 4000/,
  });
  await expect(current).toHaveAttribute("aria-pressed", "true");
  await ancestry.getByRole("button", { name: "fixture-shell 3000" }).click();
  await expect(current).toHaveAttribute("aria-pressed", "false");
  await expect(
    page.getByRole("button", { name: "Terminate parent…", exact: true }),
  ).toBeVisible();
  await current.click();
  await expect(
    page.getByRole("button", { name: "Terminate process…", exact: true }),
  ).toBeVisible();
  await ancestry.scrollIntoViewIfNeeded();
  await page.screenshot({ path: "test-results/ancestry-purple.png" });
});

test("workspace shortcuts expose hints and respect editors and dialogs", async ({
  page,
}) => {
  await page.goto("/");
  await expect(
    page.getByRole("button", { name: "Refresh", exact: true }),
  ).toHaveAttribute("title", /Ctrl.*R.*F5/);
  await page.keyboard.press("Control+3");
  await expect(
    page.getByRole("heading", { name: "Local ports" }),
  ).toBeVisible();
  await page.keyboard.press("/");
  const search = page.getByRole("textbox", { name: "Search loaded results" });
  await expect(search).toBeFocused();
  await page.keyboard.press("Control+1");
  await expect(
    page.getByRole("heading", { name: "Local ports" }),
  ).toBeVisible();
  await page.keyboard.press("Escape");
  await page.keyboard.press("Control+1");
  await expect(
    page.getByRole("heading", { name: "Processes", exact: true }),
  ).toBeVisible();
  await page.keyboard.press("Control+Tab");
  await expect(
    page.getByRole("heading", { name: "File usages", exact: true }),
  ).toBeVisible();
  await page.keyboard.press("Control+Shift+Tab");
  await page.getByRole("grid").getByRole("row").nth(1).dblclick();
  await page.keyboard.press("Control+Shift+d");
  await expect(
    page.getByRole("complementary", { name: "Process details" }),
  ).not.toBeVisible();
  await page.keyboard.press("Control+Shift+d");
  await expect(
    page.getByRole("complementary", { name: "Process details" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Terminate…", exact: true }).click();
  await page.keyboard.press("Control+3");
  await expect(
    page.getByRole("heading", { name: "Processes", exact: true }),
  ).toBeVisible();
  await page.keyboard.press("Escape");
  await page.keyboard.press("Control+,");
  await expect(
    page.getByRole("heading", { name: "Settings", exact: true }),
  ).toBeVisible();
  await page.keyboard.press("Control+f");
  await expect(search).toBeFocused();
});

for (const [locale, applyLabel] of [
  ["en", "Apply theme"],
  ["de", "Theme anwenden"],
  ["zh", "应用主题"],
]) {
  test(`first launch applies the theme without inspecting in ${locale}`, async ({
    page,
  }) => {
    await page.goto("/");
    await page.evaluate((language) => {
      localStorage.removeItem("oflh-theme");
      localStorage.setItem("oflh-language", language);
    }, locale);
    await page.reload();
    const dialog = page.getByRole("dialog");
    await expect(dialog).toBeVisible();
    await dialog.getByRole("button", { name: applyLabel, exact: true }).click();
    await expect(dialog).not.toBeVisible();
    const calls = await page.evaluate(() => (window as any).__testCalls);
    expect(
      calls.some((call: any) =>
        [
          "refresh",
          "inspect",
          "inspect_ports",
          "choose",
          "revisit",
          "follow_process",
        ].includes(call.command),
      ),
    ).toBe(false);
    await page.reload();
    await expect(dialog).not.toBeVisible();
  });
}

test("first launch previews a theme and migrates neutral Dark", async ({
  page,
}) => {
  await page.goto("/");
  await page.evaluate(() => localStorage.removeItem("oflh-theme"));
  await page.reload();
  const dialog = page.getByRole("dialog", { name: "Make OFLH yours" });
  await expect(dialog).toBeVisible();
  await expect(
    dialog.getByRole("button", { name: "VS Code Dark", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await expect(
    dialog.getByRole("button", { name: "Dark", exact: true }),
  ).toHaveCount(0);
  await dialog
    .getByRole("button", { name: "OFLH Purple", exact: true })
    .click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "purple");
  await page.screenshot({ path: "test-results/first-launch.png" });
  await dialog.getByRole("button", { name: "Apply theme" }).click();
  await page.reload();
  await expect(dialog).not.toBeVisible();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "purple");
  await page.evaluate(() => localStorage.setItem("oflh-theme", "dark"));
  await page.reload();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "vscode");
  await expect(dialog).not.toBeVisible();
});

test("single-click rows open details and panel buttons close them", async ({
  page,
}) => {
  await page.goto("/");
  const row = page.getByRole("grid").getByRole("row").nth(1);
  await expect(
    row.getByText("target/debug/fixture", { exact: true }),
  ).toBeVisible();
  await row.click();
  const panel = page.getByRole("complementary", { name: "Process details" });
  await expect(panel).toBeVisible();
  await expect(
    row.getByRole("button", { name: /^Close details/ }),
  ).toHaveAttribute("aria-expanded", "true");
  await expect(
    panel.getByRole("button", { name: /^Matching handles/ }),
  ).toBeInViewport();
  await expect(
    panel.getByRole("button", { name: /^Local ports/ }),
  ).toBeInViewport();
  await expect(
    panel.getByRole("heading", { name: "Process ancestry" }),
  ).toBeInViewport();
  await page.screenshot({ path: "test-results/details-navigation.png" });
  const file = panel.locator("section").filter({
    has: page.getByRole("heading", { name: "Selected file · full path" }),
  });
  await expect(
    file.getByText("/workspace/project/target/debug/fixture", { exact: true }),
  ).toHaveCount(1);
  await file.getByRole("button", { name: "Copy path", exact: true }).click();
  await row.getByRole("button", { name: /^Close details/ }).click();
  await expect(panel).not.toBeVisible();
  await row.click();
  await expect(panel).toBeVisible();
  const nextRow = page.getByRole("grid").getByRole("row").nth(2);
  await nextRow.click();
  await expect(
    panel.getByRole("heading", { name: "node", exact: true }),
  ).toBeVisible();
  await expect(
    row.getByRole("button", { name: /^Open details/ }),
  ).toHaveAttribute("aria-expanded", "false");
  await nextRow.getByRole("button", { name: /^Close details/ }).click();
  await expect(panel).not.toBeVisible();
  await page.keyboard.press("F5");
  const calls = await page.evaluate(() => (window as any).__testCalls);
  expect(calls.some((call: any) => call.command === "refresh")).toBe(true);
  expect(
    calls.find((call: any) => call.command === "copy").args.reference,
  ).toBe("1:0:exe");
});

test("Donate explains both support options and opens the selected destination", async ({
  page,
}) => {
  await page.goto("/");
  await expect(
    page.getByRole("button", { name: "Settings", exact: true }),
  ).toHaveCount(1);
  await expect(
    page
      .getByRole("banner")
      .getByRole("button", { name: "Settings", exact: true }),
  ).toHaveCount(0);
  await expect(
    page
      .getByRole("contentinfo")
      .getByRole("button", { name: "Karim Zouine", exact: true }),
  ).toBeVisible();
  const footer = page.getByRole("contentinfo");
  await footer
    .getByRole("button", { name: "Karim Zouine", exact: true })
    .click();
  await footer.getByRole("button", { name: "Donate", exact: true }).click();
  let support = page.getByRole("dialog", { name: "Support OFLH" });
  await expect(support).toContainText("Good for");
  await expect(support).toContainText("Trade-off");
  await expect(support).toContainText("company support");
  await support
    .getByRole("button", { name: "Continue with Buy Me a Coffee" })
    .click();
  await footer.getByRole("button", { name: "Donate", exact: true }).click();
  support = page.getByRole("dialog", { name: "Support OFLH" });
  await support
    .getByRole("button", { name: "Continue to GitHub Sponsors" })
    .click();
  await page.keyboard.press("Control+,");
  await expect(page.getByText(/There is no company behind it/)).toBeVisible();
  await page.getByRole("button", { name: "View project on GitHub" }).click();
  await page
    .getByRole("main")
    .getByRole("button", { name: "Karim Zouine", exact: true })
    .click();
  await expect(
    page.getByRole("navigation").getByRole("button", { name: "Donate" }),
  ).toHaveCount(0);
  const calls = await page.evaluate(() => (window as any).__testCalls);
  expect(
    calls.filter((call: any) => call.command === "open_donation"),
  ).toHaveLength(1);
  expect(calls.some((call: any) => call.command === "open_project")).toBe(true);
  expect(
    calls.filter((call: any) => call.command === "open_sponsors"),
  ).toHaveLength(1);
  expect(
    calls.filter((call: any) => call.command === "open_profile"),
  ).toHaveLength(2);
});

test("About links the installed release and shows a pipeline only for RC builds", async ({
  page,
}) => {
  await page.goto("/?version=0.4.0");
  await page.keyboard.press("Control+,");
  await page.getByRole("button", { name: "Installed version 0.4.0" }).click();
  await expect(
    page.getByRole("button", { name: "View RC pipeline" }),
  ).toHaveCount(0);
  await expect(
    page.getByText("Search and inspection", { exact: true }),
  ).toHaveCount(0);
  await expect(page.getByText("Commit 0123456789ab")).toHaveCount(0);
  expect(
    await page.evaluate(() =>
      (window as any).__testCalls.some(
        (call: any) => call.command === "open_installed_release",
      ),
    ),
  ).toBe(true);
  await page.goto("/?version=0.5.0-rc.1");
  await page.keyboard.press("Control+,");
  await page.getByRole("button", { name: "View RC pipeline" }).click();
  await expect(
    page.getByRole("button", { name: "View pull request" }),
  ).toHaveCount(0);
  expect(
    await page.evaluate(() =>
      (window as any).__testCalls.some(
        (call: any) => call.command === "open_build",
      ),
    ),
  ).toBe(true);
});

test("developer settings can preview the diagnostic lightbox", async ({
  page,
}) => {
  await page.goto("/");
  await page.keyboard.press("Control+,");
  await page.getByRole("button", { name: "Show sample error" }).click();
  const errorBanner = page.getByRole("alert");
  await expect(errorBanner).toContainText(
    "Sample file-manager operation failed",
  );
  await errorBanner.getByRole("button", { name: "Details" }).click();
  const details = page.getByRole("dialog", { name: "Operation details" });
  await expect(details.locator(".error-details-modal")).toContainText(
    "development preview",
  );
  await details.getByRole("button", { name: "Copy details" }).click();
  await expect(page.getByText("Error details copied")).toBeVisible();
});

test("termination preserves the captured owner port filter", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("grid").getByRole("row").nth(1).click();
  await page
    .getByRole("complementary", { name: "Process details" })
    .getByRole("button", { name: /^Local ports/ })
    .click();
  await expect(page.getByText("1 results", { exact: true })).toBeVisible();
  await page.evaluate(() => {
    (window as any).__terminateSuccessfully = true;
  });
  await page.getByRole("button", { name: "Terminate…", exact: true }).click();
  await page
    .getByRole("dialog")
    .getByRole("button", { name: "Terminate", exact: true })
    .click();
  await expect(page.getByText("Process exited", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Done", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Clear process filter" }),
  ).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "No matching local ports" }),
  ).toBeVisible();
  const calls = await page.evaluate(() => (window as any).__testCalls);
  expect(
    calls
      .filter((call: any) => call.command === "page" && call.args.query.ports)
      .at(-1).args.query.process_key,
  ).toBe("4000:18446744073709551615:0");
});

for (const mode of ["install", "download"] as const) {
  test(`startup update badge and menu share state with Settings (${mode})`, async ({
    page,
  }) => {
    await page.addInitScript((mode) => {
      (window as any).__updateMode = mode;
      (window as any).__updateMetadata = {
        rid: 1,
        currentVersion: "0.4.0",
        version: "9.9.9",
        rawJson: {},
      };
    }, mode);
    await page.goto("/");
    const gear = page.getByRole("button", { name: "Settings", exact: true });
    await expect(gear.locator(".update-badge")).toHaveText("1");
    await gear.click();
    const menu = page.getByRole("menu", { name: "Settings" });
    const action = "New update available";
    await expect(menu.getByRole("menuitem", { name: action })).toBeVisible();
    await page.screenshot({ path: `test-results/update-menu-${mode}.png` });
    await page.keyboard.press("Escape");
    await expect(menu).toHaveCount(0);
    await expect(gear).toBeFocused();
    await gear.click();
    await menu.getByRole("menuitem", { name: "Settings", exact: true }).click();
    await expect(page.getByText("Update available: v9.9.9")).toBeVisible();
    await page.keyboard.press("Control+1");
    await gear.click();
    await menu.getByRole("menuitem", { name: action }).click();
    const dialog = page.getByRole("dialog", { name: "New update available" });
    await expect(dialog).toBeVisible();
    const before = await page.evaluate(() => (window as any).__testCalls);
    expect(
      before.some(
        (call: any) =>
          call.command === "open_download" ||
          call.command === "plugin:updater|download_and_install",
      ),
    ).toBe(false);
    if (mode === "download")
      await expect(
        dialog.getByText(
          /Automatic updates are not available for this Linux package/,
        ),
      ).toBeVisible();
    await page.screenshot({ path: `test-results/update-dialog-${mode}.png` });
    await dialog
      .getByRole("button", {
        name:
          mode === "download" ? "Go to download page" : "Install and restart",
      })
      .click();
    await expect(dialog).toHaveCount(0);
    const calls = await page.evaluate(() => (window as any).__testCalls);
    expect(
      calls.filter((call: any) => call.command === "plugin:updater|check"),
    ).toHaveLength(1);
    if (mode === "download") {
      expect(
        calls.find((call: any) => call.command === "plugin:updater|check").args
          .target,
      ).toBe("windows-x86_64");
      expect(calls.some((call: any) => call.command === "open_download")).toBe(
        true,
      );
      expect(
        calls.some((call: any) => call.command === "plugin:resources|close"),
      ).toBe(true);
      expect(
        calls.some(
          (call: any) => call.command === "plugin:updater|download_and_install",
        ),
      ).toBe(false);
      expect(
        calls.some((call: any) => call.command === "plugin:process|restart"),
      ).toBe(false);
      await expect(gear.locator(".update-badge")).toBeVisible();
    } else {
      await expect(gear.locator(".update-badge")).toHaveCount(0);
      expect(
        calls.some(
          (call: any) => call.command === "plugin:updater|download_and_install",
        ),
      ).toBe(true);
    }
  });
}

test("Settings reports when no update is available and links to the releases page", async ({
  page,
}) => {
  await page.goto("/");
  await page.keyboard.press("Control+,");
  await expect(page.getByText("You’re up to date.")).toBeVisible();
  await page.getByRole("button", { name: "View release notes" }).click();
  const calls = await page.evaluate(() => (window as any).__testCalls);
  expect(calls.some((call: any) => call.command === "open_release_notes")).toBe(
    true,
  );
});

test("Settings offers to install an announced update and relaunches after install", async ({
  page,
}) => {
  await page.addInitScript(() => {
    (window as any).__updateMetadata = {
      rid: 1,
      currentVersion: "0.1.0",
      version: "9.9.9",
      date: "2026-01-01",
      body: "Fixture release notes",
      rawJson: {},
    };
  });
  await page.goto("/");
  await page.keyboard.press("Control+,");
  await expect(page.getByText("Update available: v9.9.9")).toBeVisible();
  await page.getByRole("button", { name: "Install and restart" }).click();
  await page
    .getByRole("dialog")
    .getByRole("button", { name: "Install and restart" })
    .click();
  await expect(
    page.getByText("Update installed. Restart OFLH to finish."),
  ).toBeVisible();
  const calls = await page.evaluate(() => (window as any).__testCalls);
  expect(
    calls.some(
      (call: any) => call.command === "plugin:updater|download_and_install",
    ),
  ).toBe(true);
  expect(
    calls.some((call: any) => call.command === "plugin:process|restart"),
  ).toBe(true);
});

test("failed update installation never restarts and can be retried", async ({
  page,
}) => {
  await page.addInitScript(() => {
    (window as any).__updateMetadata = {
      rid: 1,
      currentVersion: "1.0.0-rc.1",
      version: "9.9.9-rc.1",
      body: "Synthetic RC",
      rawJson: {},
    };
    (window as any).__updateInstallFails = true;
  });
  await page.goto("/");
  await page.keyboard.press("Control+,");
  await expect(page.getByText("Update available: v9.9.9-rc.1")).toBeVisible();
  await page.getByRole("button", { name: "Install and restart" }).click();
  await page
    .getByRole("dialog")
    .getByRole("button", { name: "Install and restart" })
    .click();
  await expect(
    page
      .getByRole("dialog")
      .getByText("Could not complete the update. Try again."),
  ).toBeVisible();
  let calls = await page.evaluate(() => (window as any).__testCalls);
  expect(
    calls.some((call: any) => call.command === "plugin:process|restart"),
  ).toBe(false);
  await page.evaluate(() => {
    (window as any).__updateInstallFails = false;
  });
  await page
    .getByRole("dialog")
    .getByRole("button", { name: "Install and restart" })
    .click();
  await expect(
    page.getByText("Update installed. Restart OFLH to finish."),
  ).toBeVisible();
  calls = await page.evaluate(() => (window as any).__testCalls);
  expect(
    calls.filter((call: any) => call.command === "plugin:process|restart"),
  ).toHaveLength(1);
});

test("update check failure offers release notes and recovers on retry", async ({
  page,
}) => {
  await page.addInitScript(() => {
    (window as any).__updateCheckFails = true;
  });
  await page.goto("/");
  await page.keyboard.press("Control+,");
  await expect(page.getByText("Could not check for updates")).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Install and restart" }),
  ).toHaveCount(0);
  await page.getByRole("button", { name: "View release notes" }).click();
  const calls = await page.evaluate(() => (window as any).__testCalls);
  expect(calls.some((call: any) => call.command === "open_release_notes")).toBe(
    true,
  );
  await page.evaluate(() => {
    (window as any).__updateCheckFails = false;
  });
  await page
    .getByRole("button", { name: "Check for updates", exact: true })
    .click();
  await expect(
    page.locator(".updates").getByText("You’re up to date."),
  ).toBeVisible();
  await expect(page.locator(".update-toast")).toHaveText(/You’re up to date/);
});

test("column filters submit typed predicates and F5 does not outline the entire grid", async ({
  page,
}) => {
  await page.goto("/");
  await page
    .getByRole("button", { name: "Column filters", exact: true })
    .click();
  const filters = page.getByRole("form", { name: "Column filters" });
  await filters.getByLabel("Process name", { exact: true }).fill("node");
  await filters.getByLabel("CPU minimum (%)", { exact: true }).fill("0");
  await filters.getByLabel("Memory minimum (MiB)", { exact: true }).fill("32");
  await filters.getByLabel("Evidence", { exact: true }).selectOption("none");
  await filters.getByRole("button", { name: "Apply filters" }).click();
  await expect(page.getByText("300 results", { exact: true })).toBeVisible();
  const appliedFilters = page.getByRole("status", {
    name: "Applied column filters",
  });
  await expect(appliedFilters).toContainText("Process: node");
  await expect(filters.getByRole("button", { name: "Close" })).toBeDisabled();
  await expect(
    page.getByRole("button", { name: /Column filters/ }),
  ).toBeDisabled();
  await page.screenshot({ path: "test-results/column-filters.png" });
  const calls = await page.evaluate(() => (window as any).__testCalls);
  expect(
    calls.filter((call: any) => call.command === "page").at(-1).args.query
      .columns,
  ).toMatchObject({
    name: "node",
    cpu_min: 0,
    memory_min: 32,
    evidence: "none",
  });
  await filters.getByLabel("Process name", { exact: true }).fill("no-match");
  await filters.getByRole("button", { name: "Apply filters" }).click();
  await expect(
    page.getByRole("heading", { name: "No matching processes" }),
  ).toBeVisible();
  await expect(appliedFilters).toContainText("Process: no-match");
  await appliedFilters.getByRole("button", { name: "Clear filters" }).click();
  await expect(appliedFilters).toHaveCount(0);
  await expect(page.getByText("1500 results", { exact: true })).toBeVisible();
  await filters.getByRole("button", { name: "Close", exact: true }).click();
  const grid = page.getByRole("grid");
  await grid.focus();
  await page.keyboard.press("F5");
  await expect(grid).toHaveCSS("outline-style", "none");
  await page.keyboard.press("ArrowDown");
  await expect(grid.locator('[data-cursor="true"]')).toHaveCSS(
    "outline-style",
    "solid",
  );
});

for (const [saved, resolved, background] of [
  ["rider", "rider", "rgb(23, 25, 30)"],
  ["purple", "purple", "rgb(32, 32, 43)"],
  ["light", "light", "rgb(250, 251, 252)"],
  ["dark", "vscode", "rgb(30, 30, 30)"],
  ["invalid", "vscode", "rgb(30, 30, 30)"],
  ["system", "vscode", "rgb(30, 30, 30)"],
]) {
  test(`saved ${saved} theme paints before React loads`, async ({ page }) => {
    await page.emulateMedia({ colorScheme: "dark" });
    await page.addInitScript(
      (theme) => localStorage.setItem("oflh-theme", theme),
      saved,
    );
    await page.route("**/src/main.tsx", (route) => route.abort());
    await page.goto("/");
    await expect(page.locator("html")).toHaveAttribute("data-theme", resolved);
    await expect(page.locator("html")).toHaveCSS(
      "background-color",
      background,
    );
    await expect(page.locator("#root")).toBeEmpty();
  });
}

test("column dividers remain visible without hover and resize with keyboard", async ({
  page,
}) => {
  await page.goto("/");
  const divider = page.getByRole("separator", {
    name: "Resize Process column",
    exact: true,
  });
  await expect(divider).toHaveCSS("width", "12px");
  expect(
    await divider.evaluate(
      (element) => getComputedStyle(element, "::after").backgroundColor,
    ),
  ).not.toBe("rgba(0, 0, 0, 0)");
  const before = await divider.boundingBox();
  await divider.focus();
  await page.keyboard.press("ArrowRight");
  const after = await divider.boundingBox();
  expect(after!.x).toBeGreaterThan(before!.x);
  const widthBeforeDrag = Number(await divider.getAttribute("aria-valuenow"));
  const dragBounds = await divider.boundingBox();
  await page.mouse.move(
    dragBounds!.x + dragBounds!.width / 2,
    dragBounds!.y + dragBounds!.height / 2,
  );
  await page.mouse.down();
  await page.mouse.move(
    dragBounds!.x + dragBounds!.width / 2 + 36,
    dragBounds!.y + dragBounds!.height / 2,
  );
  await page.mouse.up();
  await expect
    .poll(async () => Number(await divider.getAttribute("aria-valuenow")))
    .toBeGreaterThan(widthBeforeDrag);
});

test("documentation screenshots use only synthetic inspection data", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1600, height: 1000 });
  await page.goto("/");
  await page.keyboard.press("Control+,");
  await page.getByRole("button", { name: "VS Code Dark", exact: true }).click();
  await page.getByRole("button", { name: /^Processes/ }).click();
  await page.getByRole("grid").getByRole("row").nth(1).click();
  await expect(
    page.getByRole("complementary", { name: "Process details" }),
  ).toBeVisible();
  await page.mouse.move(0, 0);
  await page.screenshot({ path: documentationScreenshotPath("desktop.png") });

  await page.goto("/?empty-target");
  await expect(
    page.getByRole("heading", { name: "A clear view of files in use." }),
  ).toBeVisible();
  await page.evaluate(() =>
    (window as any).__emitTestEvent("drag-active", true),
  );
  await expect(
    page.getByRole("heading", { name: "Drop file or folder to inspect" }),
  ).toBeVisible();
  await page.evaluate(() =>
    (window as any).__emitTestEvent("drag-active", false),
  );
  await page.evaluate(() =>
    (window as any).__emitTestEvent("target-dropped", null),
  );
  await page.evaluate(() =>
    (window as any).__emitTestEvent("scan-status", {
      generation: 2,
      revision: 1,
      scanning: false,
      target: "/workspace/project",
      processes: 1500,
      ports: 2,
      usages: 18000,
      warnings: [],
      error: null,
      version:
        new URL(location.href).searchParams.get("version") ?? "development",
      commit: "0123456789abcdef0123456789abcdef01234567",
      build_url:
        "https://github.com/karimz1/open-file-lock-handle/actions/runs/1234567890",
      pull_request_url:
        "https://github.com/karimz1/open-file-lock-handle/pull/42",
    }),
  );
  await expect(
    page.getByRole("textbox", { name: "Target file or folder path" }),
  ).toHaveValue("/workspace/project");
  await expect(page.getByText("1500 results", { exact: true })).toBeVisible();
  await page.getByRole("grid").getByRole("row").nth(1).click();
  await expect(
    page.getByRole("complementary", { name: "Process details" }),
  ).toBeVisible();
  await page.mouse.move(0, 0);
  await page.screenshot({
    path: documentationScreenshotPath("desktop-folder.png"),
  });

  await page.goto("/");
  await page
    .getByRole("textbox", { name: "Search loaded results" })
    .fill("node");
  await expect(page.getByText("300 results", { exact: true })).toBeVisible();
  await page.screenshot({
    path: documentationScreenshotPath("desktop-search.png"),
  });

  await page.goto("/");
  await page
    .getByRole("button", { name: "Column filters", exact: true })
    .click();
  const filters = page.getByRole("form", { name: "Column filters" });
  await filters.getByLabel("Process name", { exact: true }).fill("node");
  await filters.getByRole("button", { name: "Apply filters" }).click();
  await expect(page.getByText("300 results", { exact: true })).toBeVisible();
  await page.screenshot({
    path: documentationScreenshotPath("desktop-filters.png"),
  });

  await page.goto("/");
  await page.getByRole("button", { name: /^Ports/ }).click();
  const ports = page.getByRole("grid", {
    name: "Local TCP listeners and UDP bindings",
  });
  await expect(ports.getByText("8080", { exact: true })).toBeVisible();
  await expect(ports.getByText("BOUND", { exact: true })).toBeVisible();
  await page.screenshot({
    path: documentationScreenshotPath("desktop-ports.png"),
  });
});

test("gear stays last in the sidebar and support actions fit the minimum window footer", async ({
  page,
}) => {
  await page.setViewportSize({ width: 860, height: 560 });
  await page.goto("/");
  const footer = page.getByRole("contentinfo");
  const footerBounds = await footer.evaluate((element: HTMLElement) => ({
    clientWidth: element.clientWidth,
    scrollWidth: element.scrollWidth,
  }));
  expect(footerBounds.scrollWidth).toBeLessThanOrEqual(
    footerBounds.clientWidth,
  );
  await page.getByText("Columns", { exact: true }).click();
  const columnMenu = page.getByRole("group", { name: "Visible columns" });
  const menuBounds = await columnMenu.boundingBox();
  expect(menuBounds!.x + menuBounds!.width).toBeLessThanOrEqual(860);
  await page.getByText("Columns", { exact: true }).click();
  for (const name of ["Settings"]) {
    const button = page
      .getByRole("navigation")
      .getByRole("button", { name, exact: true });
    await expect(button).toBeVisible();
    await expect(
      page.getByRole("navigation").getByRole("button", { name, exact: true }),
    ).toBeVisible();
    await expect(
      page.getByRole("banner").getByRole("button", { name, exact: true }),
    ).toHaveCount(0);
    const box = await button.boundingBox();
    expect(box!.x).toBeGreaterThanOrEqual(0);
    expect(box!.x + box!.width).toBeLessThanOrEqual(860);
  }
  const sidebar = page.getByRole("navigation");
  await expect(sidebar.locator(".sidebar-bottom > :last-child")).toHaveClass(
    "settings-menu",
  );
  const gear = sidebar.getByRole("button", { name: "Settings", exact: true });
  const star = footer.getByRole("button", {
    name: "Star on GitHub",
    exact: true,
  });
  await expect(star).toBeVisible();
  await expect(
    sidebar.getByRole("button", { name: "Star on GitHub", exact: true }),
  ).toHaveCount(0);
  const gearBounds = await gear.boundingBox();
  const starBounds = await star.boundingBox();
  const footerBox = await footer.boundingBox();
  expect(gearBounds!.y + gearBounds!.height).toBeLessThanOrEqual(footerBox!.y);
  expect(starBounds!.y).toBeGreaterThanOrEqual(footerBox!.y);
  expect(starBounds!.x).toBeGreaterThan(860 / 2);
  expect(starBounds!.x + starBounds!.width).toBeLessThanOrEqual(860);
  await expect(sidebar.getByText("Development", { exact: true })).toHaveCount(
    0,
  );
  await gear.click();
  const menu = page.getByRole("menu", { name: "Settings", exact: true });
  const popupBounds = await menu.boundingBox();
  expect(popupBounds!.y).toBeGreaterThanOrEqual(0);
  expect(popupBounds!.y + popupBounds!.height).toBeLessThanOrEqual(
    gearBounds!.y + gearBounds!.height,
  );
  const headerBounds = await page.getByRole("banner").boundingBox();
  expect(headerBounds!.y).toBe(0);
  await page.screenshot({ path: "test-results/sidebar-bottom-gear.png" });
  await page.keyboard.press("Escape");
  await sidebar
    .getByRole("button", { name: "Collapse sidebar", exact: true })
    .click();
  await expect(sidebar.locator(".sidebar-bottom > :last-child")).toHaveClass(
    "settings-menu",
  );
  await expect(gear).toBeVisible();
  await gear.click();
  await expect(menu).toBeVisible();
  await page.screenshot({
    path: "test-results/sidebar-bottom-gear-collapsed.png",
  });
  await menu.getByRole("menuitem", { name: "Settings", exact: true }).click();
  await expect(
    page.getByRole("button", {
      name: "Installed version development",
      exact: true,
    }),
  ).toBeAttached();
  await expect(
    page.getByRole("navigation").getByRole("button", { name: "Donate" }),
  ).toHaveCount(0);
  await expect(
    footer.getByRole("button", { name: "Donate", exact: true }),
  ).toBeVisible();
});

test("system language selects German and unsupported languages fall back to English", async ({
  page,
}) => {
  await page.addInitScript(() => {
    Object.defineProperty(navigator, "language", {
      configurable: true,
      value: "de-DE",
    });
  });
  await page.goto("/");
  await expect(page.locator("html")).toHaveAttribute("lang", "de");
  await expect(
    page.getByRole("button", { name: "Einstellungen", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("grid", { name: "Prozesse, die dieses Ziel verwenden" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Spaltenfilter" }).click();
  const germanFilters = page.getByRole("form", { name: "Spaltenfilter" });
  await expect(germanFilters.getByLabel("Prozessname")).toBeVisible();

  await page.addInitScript(() => {
    Object.defineProperty(navigator, "language", {
      configurable: true,
      value: "pt-PT",
    });
  });
  await page.reload();
  await expect(page.locator("html")).toHaveAttribute("lang", "en");
  await expect(
    page.getByRole("button", { name: "Settings", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("grid", { name: "Processes using this target" }),
  ).toBeVisible();
});

test("language setting switches languages and persists the preference", async ({
  page,
}) => {
  await page.goto("/");
  await page.keyboard.press("Control+,");
  await page.getByLabel("Language", { exact: true }).selectOption("de");

  await expect(page.locator("html")).toHaveAttribute("lang", "de");
  await expect(
    page.getByRole("button", { name: "Einstellungen", exact: true }),
  ).toBeVisible();
  await page.keyboard.press("Control+,");
  const language = page.getByLabel("Sprache", { exact: true });
  await expect(language).toHaveValue("de");
  await page.reload();
  await expect(page.locator("html")).toHaveAttribute("lang", "de");
  await page.keyboard.press("Control+,");
  await page.getByLabel("Sprache", { exact: true }).selectOption("system");

  await expect(page.locator("html")).toHaveAttribute("lang", "en");
  await expect(
    page.getByRole("button", { name: "Settings", exact: true }),
  ).toBeVisible();
  expect(await page.evaluate(() => localStorage.getItem("oflh-language"))).toBe(
    "system",
  );
});

test("Chinese system language translates the workspace and process confirmation", async ({
  page,
}) => {
  await page.addInitScript(() => {
    Object.defineProperty(navigator, "language", {
      configurable: true,
      value: "zh-CN",
    });
  });
  await page.goto("/");
  await expect(page.locator("html")).toHaveAttribute("lang", "zh");
  await expect(
    page.getByRole("grid", { name: "使用此目标的进程" }),
  ).toBeVisible();
  await expect(page.locator(".evidence-note")).toContainText(
    "文件被使用不代表被锁定",
  );
  await expect(page.locator(".evidence-note summary")).toContainText(
    "检测范围说明",
  );
  await page.getByRole("button", { name: "列筛选", exact: true }).click();
  const filters = page.getByRole("form", { name: "列筛选" });
  await expect(filters.getByLabel("进程名称")).toBeVisible();
  await page.screenshot({ path: "test-results/chinese-filters.png" });
  await filters.getByRole("button", { name: "关闭", exact: true }).click();
  await page.getByRole("row").filter({ hasText: "Code" }).first().click();
  await page.getByRole("button", { name: "终止…", exact: true }).click();
  const confirmation = page.getByRole("dialog");
  await expect(confirmation).toContainText("未保存的工作可能丢失");
  await expect(
    confirmation.getByRole("button", { name: "取消", exact: true }),
  ).toBeFocused();
  await page.screenshot({ path: "test-results/chinese-confirmation.png" });
});

test("Chinese language preference survives reload and can return to system default", async ({
  page,
}) => {
  await page.goto("/");
  await page.keyboard.press("Control+,");
  await page.getByLabel("Language", { exact: true }).selectOption("zh");
  await expect(page.locator("html")).toHaveAttribute("lang", "zh");
  expect(await page.evaluate(() => localStorage.getItem("oflh-language"))).toBe(
    "zh",
  );
  await page.reload();
  await expect(page.locator("html")).toHaveAttribute("lang", "zh");
  await page.keyboard.press("Control+,");
  await expect(page.getByLabel("语言", { exact: true })).toHaveValue("zh");
  await expect(
    page.getByRole("option", { name: "简体中文", exact: true }),
  ).toHaveCount(1);
  await expect(
    page.getByRole("heading", { name: "更新", exact: true }),
  ).toBeVisible();
  await expect(page.getByText("已是最新版本。", { exact: true })).toBeVisible();
  await page.screenshot({ path: "test-results/chinese-settings.png" });
  await page.getByLabel("语言", { exact: true }).selectOption("system");
  await expect(page.locator("html")).toHaveAttribute("lang", "en");
  expect(await page.evaluate(() => localStorage.getItem("oflh-language"))).toBe(
    "system",
  );
});

test("normal termination recovery confirms captured targets instead of refreshing again", async ({
  page,
}) => {
  await page.goto("/");
  await expect(page.getByText("1500 results")).toBeVisible();
  await page.evaluate(() => {
    (window as any).__terminationResults = [
      { pid: 4000, outcome: "still_running", error: null },
    ];
  });
  await page.getByRole("grid").getByRole("row").nth(1).click();
  await page
    .getByRole("textbox", { name: "Search loaded results" })
    .fill("node");
  await page.getByRole("button", { name: "Terminate…", exact: true }).click();
  const dialog = page.getByRole("dialog");
  await dialog.getByRole("button", { name: "Terminate", exact: true }).click();
  await expect(
    dialog.getByText(/Some processes could not be stopped normally/),
  ).toBeVisible();
  await expect(
    dialog.getByRole("button", { name: "Refresh again" }),
  ).toHaveCount(0);
  await page.screenshot({ path: "test-results/force-recovery-results.png" });
  await dialog
    .getByRole("button", { name: "Force terminate…", exact: true })
    .click();
  await expect(
    dialog.getByRole("button", { name: "Cancel", exact: true }),
  ).toBeFocused();
  await expect(dialog.getByText("PID 4000")).toBeVisible();
  await expect(
    dialog.getByText(/without allowing normal cleanup/),
  ).toBeVisible();
  const calls = await page.evaluate(() => (window as any).__testCalls);
  expect(
    calls
      .filter((call: any) => call.command === "prepare")
      .map((call: any) => call.args),
  ).toEqual([
    { revision: 1, keys: ["4000:18446744073709551615:0"], force: false },
    { revision: 1, keys: ["4000:18446744073709551615:0"], force: true },
  ]);
  expect(
    calls.filter((call: any) => call.command === "terminate"),
  ).toHaveLength(1);
});

test("normal ancestor failure offers force recovery for the captured ancestor", async ({
  page,
}) => {
  await page.goto("/");
  await expect(page.getByText("1500 results")).toBeVisible();
  await page.evaluate(() => {
    (window as any).__terminationResults = [
      {
        pid: 3000,
        outcome: "failed",
        error: {
          kind: "permission_denied",
          message: "fixture permission failure",
          os_code: 13,
        },
      },
    ];
  });
  await page.getByRole("grid").getByRole("row").nth(1).dblclick();
  await page.getByRole("button", { name: "fixture-shell 3000" }).click();
  await page
    .getByRole("button", { name: "Terminate parent…", exact: true })
    .click();
  const dialog = page.getByRole("dialog");
  await dialog.getByRole("button", { name: "Terminate", exact: true }).click();
  await expect(dialog.getByText("fixture permission failure")).toBeVisible();
  await dialog
    .getByRole("button", { name: "Force terminate…", exact: true })
    .click();
  await expect(
    dialog.getByRole("button", { name: "Cancel", exact: true }),
  ).toBeFocused();
  await expect(dialog.getByText("PID 3000")).toBeVisible();
  const calls = await page.evaluate(() => (window as any).__testCalls);
  expect(
    calls.filter((call: any) => call.command === "prepare_ancestor").at(-1)
      .args,
  ).toEqual({
    owner: "4000:18446744073709551615:0",
    key: "3000:900:0",
    force: true,
  });
});

for (const force of [false, true]) {
  test(`permission recovery preserves ${force ? "force" : "normal"} mode and defaults to cancel`, async ({
    page,
  }) => {
    await page.addInitScript((force) => {
      (window as any).__adminForce = force;
      (window as any).__terminationResults = [
        {
          pid: 4000,
          outcome: "failed",
          admin_recovery: true,
          error: {
            kind: "permission_denied",
            message: "Synthetic permission denied",
            os_code: 13,
          },
        },
      ];
    }, force);
    await page.goto("/");
    await page.getByRole("grid").getByRole("row").nth(1).click();
    await page
      .getByRole("button", {
        name: force ? "Force terminate…" : "Terminate…",
        exact: true,
      })
      .click();
    await page
      .getByRole("dialog")
      .getByRole("button", {
        name: force ? "Force terminate" : "Terminate",
        exact: true,
      })
      .click();
    await page
      .getByRole("button", { name: "Retry with administrator privileges…" })
      .click();
    const dialog = page.getByRole("dialog");
    await expect(
      dialog.getByRole("button", { name: "Cancel", exact: true }),
    ).toBeFocused();
    await expect(dialog.getByText("PID 4000")).toBeVisible();
    await expect(
      dialog.getByText(
        /operating system will request administrator authorization/,
      ),
    ).toBeVisible();
    await page.screenshot({
      path: `test-results/admin-confirmation-${force}.png`,
    });
    await page.keyboard.press("Escape");
    let calls = await page.evaluate(() => (window as any).__testCalls);
    expect(
      calls.filter((call: any) => call.command === "terminate"),
    ).toHaveLength(1);
    expect(
      calls.find((call: any) => call.command === "prepare_elevated").args,
    ).toEqual({ ticket: "captured-ticket" });
  });
}

test("confirmed administrator retry uses its new receipt and never offers repeated elevation", async ({
  page,
}) => {
  await page.addInitScript(() => {
    (window as any).__terminationResults = [
      {
        pid: 4000,
        outcome: "failed",
        admin_recovery: true,
        error: {
          kind: "permission_denied",
          message: "Synthetic permission denied",
          os_code: 13,
        },
      },
    ];
  });
  await page.goto("/");
  await page.getByRole("grid").getByRole("row").nth(1).click();
  await page
    .getByRole("button", { name: "Force terminate…", exact: true })
    .click();
  await page
    .getByRole("dialog")
    .getByRole("button", { name: "Force terminate", exact: true })
    .click();
  await page
    .getByRole("button", { name: "Retry with administrator privileges…" })
    .click();
  await page.evaluate(() => {
    (window as any).__terminationResults = [
      {
        pid: 4000,
        outcome: "failed",
        admin_recovery: false,
        error: {
          kind: "permission_denied",
          message: "Synthetic elevated permission denied",
          os_code: 13,
        },
      },
    ];
  });
  await page
    .getByRole("dialog")
    .getByRole("button", { name: "Force terminate", exact: true })
    .click();
  await expect(
    page.getByText("Synthetic elevated permission denied"),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Retry with administrator privileges…" }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("dialog").getByRole("button", { name: "Force terminate…" }),
  ).toHaveCount(0);
  const calls = await page.evaluate(() => (window as any).__testCalls);
  expect(
    calls
      .filter((call: any) => call.command === "terminate")
      .map((call: any) => call.args),
  ).toEqual([{ ticket: "captured-ticket" }, { ticket: "admin-ticket" }]);
});

test("gear is icon-only and themes submenu supports keyboard selection and persistence", async ({
  page,
}) => {
  await page.goto("/");
  const gear = page.getByRole("button", { name: "Settings", exact: true });
  await expect(gear).toHaveText("");
  await gear.click();
  const menu = page.getByRole("menu", { name: "Settings", exact: true });
  await expect(
    menu.getByRole("menuitem", { name: "Settings", exact: true }),
  ).toBeFocused();
  await page.keyboard.press("ArrowDown");
  const themes = menu.getByRole("menuitem", { name: "Themes", exact: true });
  await expect(themes).toBeFocused();
  await page.keyboard.press("ArrowRight");
  const submenu = page.getByRole("menu", { name: "Themes", exact: true });
  await expect(
    submenu.getByRole("menuitemradio", { name: "Light", exact: true }),
  ).toBeFocused();
  await expect(submenu.getByRole("menuitemradio")).toHaveCount(4);
  await page.screenshot({ path: "test-results/settings-themes-menu.png" });
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("Enter");
  await expect(menu).toHaveCount(0);
  await expect(gear).toBeFocused();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "rider");
  await page.reload();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "rider");
  await gear.click();
  await themes.hover();
  await expect(submenu).toBeVisible();
  await submenu
    .getByRole("menuitemradio", { name: "VS Code Dark", exact: true })
    .hover();
  await page.screenshot({ path: "test-results/settings-themes-dark.png" });
  await submenu
    .getByRole("menuitemradio", { name: "Rider Dark", exact: true })
    .focus();
  await page.keyboard.press("Escape");
  await expect(submenu).toHaveCount(0);
  await expect(themes).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(menu).toHaveCount(0);
});

test("manual update checks show a latest-version toast and failure feedback outside Settings", async ({
  page,
}) => {
  await page.goto("/");
  const gear = page.getByRole("button", { name: "Settings", exact: true });
  await expect(page.locator(".update-toast")).toHaveCount(0);
  await gear.click();
  await page
    .getByRole("menuitem", { name: "Check for updates", exact: true })
    .click();
  await expect(page.locator(".update-toast")).toHaveText(/You’re up to date/);
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await page.evaluate(() => {
    (window as any).__updateCheckFails = true;
  });
  await gear.click();
  await page
    .getByRole("menuitem", { name: "Check for updates", exact: true })
    .click();
  await expect(page.locator(".update-toast")).toHaveText(
    /Could not check for updates/,
  );
});

test("hourly background checks discover updates quietly and retain the badge on network failure", async ({
  page,
}) => {
  await page.clock.install();
  await page.goto("/");
  const gear = page.getByRole("button", { name: "Settings", exact: true });
  await expect
    .poll(async () =>
      page.evaluate(
        () =>
          (window as any).__testCalls.filter(
            (call: any) => call.command === "plugin:updater|check",
          ).length,
      ),
    )
    .toBe(1);
  await expect(gear.locator(".update-badge")).toHaveCount(0);
  await page.evaluate(() => {
    (window as any).__updateMetadata = {
      rid: 1,
      currentVersion: "0.5.0",
      version: "9.9.9",
      rawJson: {},
    };
  });
  await page.clock.fastForward(60 * 60 * 1000);
  await expect(gear.locator(".update-badge")).toHaveText("1");
  await expect(page.locator(".update-toast")).toHaveCount(0);
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await page.evaluate(() => {
    (window as any).__updateCheckFails = true;
  });
  await page.clock.fastForward(60 * 60 * 1000);
  await expect
    .poll(async () =>
      page.evaluate(
        () =>
          (window as any).__testCalls.filter(
            (call: any) => call.command === "plugin:updater|check",
          ).length,
      ),
    )
    .toBe(3);
  await expect(gear.locator(".update-badge")).toHaveText("1");
  await expect(page.locator(".update-toast")).toHaveCount(0);
  await gear.click();
  await page
    .getByRole("menuitem", { name: "New update available", exact: true })
    .click();
  await expect(page.getByRole("dialog")).toBeVisible();
  await page.clock.fastForward(2 * 60 * 60 * 1000);
  expect(
    await page.evaluate(
      () =>
        (window as any).__testCalls.filter(
          (call: any) => call.command === "plugin:updater|check",
        ).length,
    ),
  ).toBe(3);
  await page.getByRole("dialog").getByRole("button", { name: "Later" }).click();
  await expect(gear.locator(".update-badge")).toHaveText("1");
});

test("manual check opens update confirmation and release notes before installation", async ({
  page,
}) => {
  await page.goto("/");
  await page.evaluate(() => {
    (window as any).__updateMetadata = {
      rid: 1,
      currentVersion: "0.5.0",
      version: "9.9.9",
      rawJson: {},
    };
  });
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await page
    .getByRole("menuitem", { name: "Check for updates", exact: true })
    .click();
  const dialog = page.getByRole("dialog", { name: "New update available" });
  await expect(dialog).toBeVisible();
  await expect(
    dialog.getByText(/bug fixes, stability improvements, and new features/),
  ).toBeVisible();
  await expect(
    dialog.getByText(/I recommend installing the latest version/),
  ).toBeVisible();
  await page.screenshot({ path: "test-results/update-confirmation.png" });
  await dialog.getByRole("button", { name: "View release notes" }).click();
  const calls = await page.evaluate(() => (window as any).__testCalls);
  expect(calls.some((call: any) => call.command === "open_release_notes")).toBe(
    true,
  );
  expect(
    calls.some(
      (call: any) => call.command === "plugin:updater|download_and_install",
    ),
  ).toBe(false);
  await dialog.getByRole("button", { name: "Later" }).click();
  await expect(
    page
      .getByRole("button", { name: "Settings", exact: true })
      .locator(".update-badge"),
  ).toHaveText("1");
});

test("pointer-opened menus and termination dialogs do not pre-highlight actions, while keyboard focus stays visible", async ({
  page,
}) => {
  await page.goto("/");
  const gear = page.getByRole("button", { name: "Settings", exact: true });
  await gear.click();
  const menu = page.getByRole("menu", { name: "Settings", exact: true });
  const settings = menu.getByRole("menuitem", {
    name: "Settings",
    exact: true,
  });
  await expect(settings).toBeFocused();
  await expect(settings).toHaveCSS("outline-style", "none");
  const themes = menu.getByRole("menuitem", { name: "Themes", exact: true });
  await themes.hover();
  await expect(themes).toHaveCSS("outline-style", "none");
  await themes.click();
  const light = page.getByRole("menuitemradio", { name: "Light", exact: true });
  await expect(light).toBeFocused();
  await expect(light).toHaveCSS("outline-style", "none");
  await page.keyboard.press("ArrowDown");
  const rider = page.getByRole("menuitemradio", {
    name: "Rider Dark",
    exact: true,
  });
  await expect(rider).toBeFocused();
  await expect(rider).toHaveCSS("outline-style", "solid");
  await page.keyboard.press("Escape");
  await page.keyboard.press("Escape");
  await page.getByRole("grid").getByRole("row").nth(1).click();
  await page
    .getByRole("button", { name: "Force terminate…", exact: true })
    .click();
  const dialog = page.getByRole("dialog");
  const cancel = dialog.getByRole("button", { name: "Cancel", exact: true });
  await expect(cancel).toBeFocused();
  await expect(cancel).toHaveCSS("outline-style", "none");
  await page.screenshot({
    path: "test-results/pointer-termination-dialog.png",
  });
  await page.keyboard.press("Tab");
  const terminate = dialog.getByRole("button", {
    name: "Force terminate",
    exact: true,
  });
  await expect(terminate).toBeFocused();
  await expect(terminate).toHaveCSS("outline-style", "solid");
  await page.keyboard.press("Shift+Tab");
  await expect(cancel).toBeFocused();
  await expect(cancel).toHaveCSS("outline-style", "solid");
  await page.keyboard.press("Enter");
  await expect(dialog).toHaveCount(0);
  const calls = await page.evaluate(() => (window as any).__testCalls);
  expect(calls.some((call: any) => call.command === "terminate")).toBe(false);
});

test("System highlights the concrete theme and follows OS changes in cards and the menu", async ({
  page,
}) => {
  await page.emulateMedia({ colorScheme: "dark" });
  await page.goto("/");
  await page.keyboard.press("Control+,");
  const system = page.getByRole("button", { name: "System", exact: true });
  const vscode = page.getByRole("button", {
    name: "VS Code Dark",
    exact: true,
  });
  const light = page.getByRole("button", { name: "Light", exact: true });
  await system.click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "vscode");
  await expect(system).toHaveAttribute("aria-pressed", "true");
  await expect(vscode).toHaveAttribute("aria-pressed", "true");
  await expect(vscode).toHaveClass(/active/);
  await expect(system).not.toHaveClass(/active/);
  await expect(vscode.getByText("Used by System")).toBeVisible();
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await page.getByRole("menuitem", { name: "Themes", exact: true }).hover();
  const submenu = page.getByRole("menu", { name: "Themes", exact: true });
  const systemMode = submenu.getByRole("menuitemcheckbox", {
    name: "System",
    exact: true,
  });
  const vscodeItem = submenu.getByRole("menuitemradio", {
    name: "VS Code Dark",
    exact: true,
  });
  const lightItem = submenu.getByRole("menuitemradio", {
    name: "Light",
    exact: true,
  });
  await expect(systemMode).toHaveAttribute("aria-checked", "true");
  await expect(vscodeItem).toHaveAttribute("aria-checked", "true");
  await expect(
    submenu.getByRole("menuitemradio", { checked: true }),
  ).toHaveCount(1);
  await page.screenshot({ path: "test-results/system-theme-dark.png" });
  await page.emulateMedia({ colorScheme: "light" });
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  await expect(systemMode).toHaveAttribute("aria-checked", "true");
  await expect(vscodeItem).toHaveAttribute("aria-checked", "false");
  await expect(lightItem).toHaveAttribute("aria-checked", "true");
  await expect(
    submenu.getByRole("menuitemradio", { checked: true }),
  ).toHaveCount(1);
  await expect(light).toHaveAttribute("aria-pressed", "true");
  await expect(light.getByText("Used by System")).toBeVisible();
  await lightItem.click();
  expect(await page.evaluate(() => localStorage.getItem("oflh-theme"))).toBe(
    "light",
  );
  await page.emulateMedia({ colorScheme: "dark" });
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  await page.reload();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
});

test("System can be enabled from the menu and persists across launches", async ({
  page,
}) => {
  await page.emulateMedia({ colorScheme: "dark" });
  await page.goto("/");
  const gear = page.getByRole("button", { name: "Settings", exact: true });
  await gear.click();
  await page.getByRole("menuitem", { name: "Themes", exact: true }).hover();
  await page
    .getByRole("menuitemcheckbox", { name: "System", exact: true })
    .click();
  expect(await page.evaluate(() => localStorage.getItem("oflh-theme"))).toBe(
    "system",
  );
  await expect(page.locator("html")).toHaveAttribute("data-theme", "vscode");
  await page.reload();
  await gear.click();
  await page.getByRole("menuitem", { name: "Themes", exact: true }).click();
  const vscode = page.getByRole("menuitemradio", {
    name: "VS Code Dark",
    exact: true,
  });
  await expect(vscode).toBeFocused();
  await expect(vscode).toHaveAttribute("aria-checked", "true");
  await page
    .getByRole("menuitemcheckbox", { name: "System", exact: true })
    .click();
  expect(await page.evaluate(() => localStorage.getItem("oflh-theme"))).toBe(
    "vscode",
  );
  await expect(page.locator("html")).toHaveAttribute("data-theme", "vscode");
});

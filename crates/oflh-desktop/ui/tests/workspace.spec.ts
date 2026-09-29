import { test, expect } from "@playwright/test";
// Synthetic IPC fixtures are test-only. The production bundle always invokes Rust.
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
      version: "development",
    };
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
        },
        async invoke(command: string, args: Record<string, any> = {}) {
          calls.push({ command, args });
          if (command === "plugin:event|listen") return 1;
          if (command === "plugin:event|unlisten") return;
          if (command === "status") return status;
          if (command === "recent")
            return [{ id: 1, display: "/workspace/project" }];
          if (["refresh", "inspect", "revisit", "choose"].includes(command))
            return (status = {
              ...status,
              generation: status.generation + 1,
              revision: terminated ? status.revision + 1 : status.revision,
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
          if (command === "prepare_ancestor")
            return {
              ticket: "parent-ticket",
              force: args.force,
              targets: [{ key: args.key, name: "fixture-shell", pid: 3000 }],
            };
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
              "reveal",
              "open_project",
              "open_donation",
            ].includes(command)
          )
            return;
          throw new Error(`Unexpected test IPC command ${command}`);
        },
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
  await page
    .getByRole("button", { name: "Settings", exact: true })
    .first()
    .click();
  await page.getByRole("button", { name: "VS Code Dark", exact: true }).click();
  await page.getByRole("button", { name: /^Processes/ }).click();
  await expect(grid.getByText("4000", { exact: true })).toBeVisible();
  await page.screenshot({ path: "test-results/workspace-dark.png" });
  await grid.focus();
  await page.keyboard.press("End");
  await expect(grid.getByText("5499", { exact: true })).toBeVisible();
  expect(await grid.getByRole("row").count()).toBeLessThan(60);
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
  await page
    .getByRole("button", { name: "Settings", exact: true })
    .first()
    .click();
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
    await page
      .getByRole("button", { name: "Settings", exact: true })
      .first()
      .click();
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
  await dialog.getByRole("button", { name: "Start inspecting" }).click();
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

test("settings credits and donation links use the native opener", async ({
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
  await page.getByRole("button", { name: "Donate", exact: true }).click();
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await expect(
    page.getByText("Created by Karim Zouine (karimz1)."),
  ).toBeVisible();
  await page.getByRole("button", { name: "View project on GitHub" }).click();
  await page
    .getByRole("button", { name: "Buy Me a Coffee", exact: true })
    .click();
  const calls = await page.evaluate(() => (window as any).__testCalls);
  expect(
    calls.filter((call: any) => call.command === "open_donation"),
  ).toHaveLength(2);
  expect(calls.some((call: any) => call.command === "open_project")).toBe(true);
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
  await filters.getByRole("button", { name: "Clear filters" }).click();
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
  ["rider", "rider", "rgb(25, 26, 28)"],
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
  await expect(divider).toHaveCSS("border-right-style", "solid");
  await expect(divider).toHaveCSS("border-right-width", "1px");
  expect(
    await divider.evaluate(
      (element) => getComputedStyle(element).borderRightColor,
    ),
  ).not.toBe("rgba(0, 0, 0, 0)");
  const before = await divider.boundingBox();
  await divider.focus();
  await page.keyboard.press("ArrowRight");
  const after = await divider.boundingBox();
  expect(after!.x).toBeGreaterThan(before!.x);
});

test("documentation screenshot uses only synthetic inspection data", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1600, height: 1000 });
  await page.goto("/");
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await page.getByRole("button", { name: "VS Code Dark", exact: true }).click();
  await page.getByRole("button", { name: /^Processes/ }).click();
  await page.getByRole("grid").getByRole("row").nth(1).click();
  await expect(
    page.getByRole("complementary", { name: "Process details" }),
  ).toBeVisible();
  await page.mouse.move(0, 0);
  await page.screenshot({
    path:
      process.env.OFLH_UPDATE_SCREENSHOTS === "1"
        ? "../../../images/desktop.png"
        : "test-results/desktop-preview.png",
  });
});

test("utility actions stay in the sidebar and fit the minimum window", async ({
  page,
}) => {
  await page.setViewportSize({ width: 860, height: 560 });
  await page.goto("/");
  for (const name of ["Star on GitHub", "Settings", "Donate"]) {
    const button = page.getByRole("button", { name, exact: true });
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
});

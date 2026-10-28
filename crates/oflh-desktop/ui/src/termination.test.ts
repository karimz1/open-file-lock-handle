import { describe, expect, it } from "vitest";
import type { ActionResult, Confirmation, ConfirmationTarget } from "./api";
import {
  forceRecoveryTargets,
  IMPACT_SENTENCES,
  scrollFades,
  impactGroupView,
  impactRow,
  impactSummary,
} from "./termination";
import { t } from "./i18n";

const confirmation: Confirmation = {
  ticket: "fixture",
  force: false,
  elevated: false,
  targets: [
    { key: "42:10:0", name: "fixture", pid: 42 },
    { key: "43:11:0", name: "other", pid: 43 },
  ],
};
const result = (
  outcome: ActionResult["outcome"],
  kind?: string,
): ActionResult => ({
  pid: 42,
  outcome,
  error: kind ? { kind, message: "fixture failure", os_code: null } : null,
});
describe("force recovery", () => {
  it("offers only unsuccessful normal targets with captured identities", () => {
    for (const failure of [
      result("still_running"),
      result("failed", "permission_denied"),
      result("failed", "io"),
    ]) {
      expect(
        forceRecoveryTargets(confirmation, [
          failure,
          { pid: 43, outcome: "exited", error: null },
        ]),
      ).toEqual([confirmation.targets[0]]);
    }
  });
  it("never escalates force results or uncertain, stale, protected and cancelled targets", () => {
    expect(
      forceRecoveryTargets({ ...confirmation, force: true }, [
        result("still_running"),
      ]),
    ).toEqual([]);
    expect(forceRecoveryTargets(null, [result("still_running")])).toEqual([]);
    for (const outcome of ["exited", "unverified"] as const) {
      expect(forceRecoveryTargets(confirmation, [result(outcome)])).toEqual([]);
    }
    for (const kind of [
      "identity_changed",
      "protected",
      "cancelled",
      "unavailable",
      "not_found",
      "invalid_request",
    ]) {
      expect(
        forceRecoveryTargets(confirmation, [result("failed", kind)]),
      ).toEqual([]);
    }
  });
});

describe("termination impact summary", () => {
  const list = (count: number) => ({
    items: Array.from({ length: Math.min(count, 5) }, (_, n) => `item ${n}`),
    more: Math.max(0, count - 5),
  });
  const target = (impact: Partial<ConfirmationTarget["impact"]>) =>
    ({
      key: "42:10:0",
      name: "Blender",
      pid: 42,
      role: "holder",
      impact: {
        windows: null,
        children: null,
        files: list(0),
        ports: list(0),
        ...impact,
      },
    }) as ConfirmationTarget;

  it("stays silent for the expected single window, file or port", () => {
    expect(impactSummary(target({}))).toEqual([]);
    expect(impactSummary(target({ windows: list(1), files: list(1) }))).toEqual(
      [],
    );
    expect(impactSummary(target({ ports: list(1) }))).toEqual([]);
    expect(impactSummary({ key: "1:1:0", name: "legacy", pid: 1 })).toEqual([]);
  });

  it("orders plain sentences by surprise and names a window", () => {
    const parts = impactSummary(
      target({
        windows: list(7),
        children: list(1),
        files: list(3),
        ports: list(1),
      }),
    );
    expect(parts.map((part) => [part.key, part.warn])).toEqual([
      ["termination.k_impact_windows", true],
      ["termination.k_impact_children_one", false],
      ["termination.k_impact_files_other", false],
      ["termination.k_impact_ports_one", false],
    ]);
    expect(t(parts[0].key, parts[0].values)).toBe(
      "Blender closes completely, with all 7 open windows, such as “item 0”.",
    );
    expect(t(parts[2].key, parts[2].values)).toBe(
      "2 more files it has open are released too.",
    );
    expect(IMPACT_SENTENCES).toBe(2);
  });

  it("splits detail rows into a recognizable label and muted technical info", () => {
    expect(impactRow("children", "python3 (PID 4100)")).toEqual({
      primary: "python3",
      secondary: "PID 4100",
      copy: "4100",
    });
    expect(impactRow("files", "/build/textures/a.png")).toEqual({
      primary: "a.png",
      secondary: "/build/textures",
      copy: "/build/textures/a.png",
    });
    expect(impactRow("files", "C:\\Users\\k\\scene.blend").primary).toBe(
      "scene.blend",
    );
    expect(impactRow("ports", "TCP 127.0.0.1:5432")).toEqual({
      primary: "5432",
      secondary: "TCP 127.0.0.1",
    });
    expect(impactRow("ports", "UDP [::1]:53").secondary).toBe("UDP [::1]");
    expect(impactRow("ports", "TCP 3000")).toEqual({
      primary: "3000",
      secondary: "TCP",
    });
    expect(impactRow("windows", "scene.blend - Blender")).toEqual({
      primary: "scene.blend - Blender",
      secondary: "",
    });
  });
});

describe("scroll fades", () => {
  it("shows the bottom fade while more content follows", () => {
    expect(
      scrollFades({ scrollTop: 0, scrollHeight: 900, clientHeight: 300 }),
    ).toEqual({ top: false, bottom: true });
    expect(
      scrollFades({ scrollTop: 200, scrollHeight: 900, clientHeight: 300 }),
    ).toEqual({ top: true, bottom: true });
  });
  it("hides the fades at the end and when everything fits", () => {
    expect(
      scrollFades({ scrollTop: 600, scrollHeight: 900, clientHeight: 300 }),
    ).toEqual({ top: true, bottom: false });
    expect(
      scrollFades({ scrollTop: 0, scrollHeight: 300, clientHeight: 300 }),
    ).toEqual({ top: false, bottom: false });
  });
});

describe("impactGroupView", () => {
  const list = (n: number, more = 0) => ({
    items: Array.from({ length: n }, (_, i) => `scene${i + 1}.blend`),
    more,
  });
  it("previews five rows and counts the rest", () => {
    const view = impactGroupView(list(7), false);
    expect(view.items.map((row) => row.item)).toEqual([
      "scene1.blend",
      "scene2.blend",
      "scene3.blend",
      "scene4.blend",
      "scene5.blend",
    ]);
    expect(view).toMatchObject({
      hidden: 2,
      expandable: true,
      filterable: false,
    });
  });
  it("never offers to expand a short group", () => {
    expect(impactGroupView(list(5), true)).toMatchObject({
      hidden: 0,
      expandable: false,
      filterable: false,
    });
  });
  it("lists every kept row when expanded and filters long groups", () => {
    const view = impactGroupView(list(150, 3), true);
    expect(view.items).toHaveLength(150);
    expect(view).toMatchObject({ filterable: true, unlisted: 3, hidden: 0 });
    expect(impactGroupView(list(21), true).filterable).toBe(true);
    expect(impactGroupView(list(20), true).filterable).toBe(false);
  });
  it("matches every word, ignoring case, and keeps original indexes", () => {
    const view = impactGroupView(list(150), true, "BLEND scene14");
    expect(view.items.map((row) => row.index)).toEqual([
      13,
      ...Array.from({ length: 10 }, (_, i) => 139 + i),
    ]);
    expect(impactGroupView(list(150), true, "nothing").items).toEqual([]);
  });
});

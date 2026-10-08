import { fileURLToPath } from "node:url";
import { createServer } from "vite";
import type { ViteDevServer } from "vite";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import runes, { RUNTIME_VIRTUAL_ID } from "../src/core/unplugin.ts";
import type { TraceEvent } from "../src/core/runtime.ts";

const ROOT = fileURLToPath(new URL("..", import.meta.url));

interface DataPath {
  name: string;
  value?: string;
  input: boolean;
  steps: { name: string; calls: { label: string }[]; durationNs: number }[];
  otherInputs: string[];
  totalNs: number;
  pending: boolean;
  threw: boolean;
}

type BuildDataPaths = (events: TraceEvent[]) => DataPath[];
type LayoutPathTree = (
  paths: DataPath[],
) => { path: DataPath; depth: number; last: boolean; hasChildren: boolean; guides: boolean[] }[];

const at_ns = 0;

describe("buildDataPaths", () => {
  let server: ViteDevServer;
  let buildDataPaths: BuildDataPaths;
  let layoutPathTree: LayoutPathTree;
  let counterEvents: TraceEvent[];

  beforeAll(async () => {
    server = await createServer({
      root: ROOT,
      logLevel: "silent",
      server: { middlewareMode: true },
      plugins: [runes.vite()],
    });
    // Loaded through Vite because the app module imports types from the
    // virtual runtime, which only the app's type environment declares.
    ({ buildDataPaths, layoutPathTree } = (await server.ssrLoadModule(
      "/src/app/lib/explorer/trace/trace-paths.ts",
    )) as { buildDataPaths: BuildDataPaths; layoutPathTree: LayoutPathTree });
    const counter = (await server.ssrLoadModule(
      "/src/app/lib/explorer/experiments/counter/scenario.ts",
    )) as { step: (current: number, delta: 1 | -1) => unknown };
    const runtime = (await server.ssrLoadModule(RUNTIME_VIRTUAL_ID)) as {
      clearTraceEvents: () => void;
      takeTraceEvents: () => TraceEvent[];
    };
    runtime.clearTraceEvents();
    counter.step(4, 1);
    counterEvents = runtime.takeTraceEvents();
  });

  afterAll(async () => {
    await server.close();
  });

  it("lists every value of the counter step with its path from the input", () => {
    const paths = buildDataPaths(counterEvents);
    expect(paths.map((path) => path.name)).toEqual([
      "count",
      "next",
      "doubled",
      "parity",
      "summary",
    ]);
    const byName = new Map(paths.map((path) => [path.name, path]));

    expect(byName.get("count")).toMatchObject({ input: true, value: "4", totalNs: 0 });
    expect(byName.get("doubled")?.value).toBe("10");
    expect(byName.get("doubled")?.steps.map((step) => step.name)).toEqual([
      "count",
      "next",
      "doubled",
    ]);
    expect(byName.get("next")?.steps[1].calls.map((call) => call.label)).toEqual(["increment()"]);

    // summarize(next, parity) follows the longer chain; next is already on it.
    const summary = byName.get("summary")!;
    expect(summary.steps.map((step) => step.name)).toEqual(["count", "next", "parity", "summary"]);
    expect(summary.otherInputs).toEqual([]);
    expect(summary.pending || summary.threw).toBe(false);
  });

  it("nests each value under the previous step of its path", () => {
    const rows = layoutPathTree(buildDataPaths(counterEvents));
    expect(
      rows.map((row) => [
        row.path.name,
        row.depth,
        row.last,
        row.hasChildren,
        row.guides.join(","),
      ]),
    ).toEqual([
      ["count", 0, true, true, ""],
      ["next", 1, true, true, ""],
      ["doubled", 2, false, false, "false"],
      ["parity", 2, true, true, "false"],
      ["summary", 3, true, false, "false,false"],
    ]);
  });

  it("sums measured call durations along the path", () => {
    const durations = new Map(
      counterEvents.flatMap((event) =>
        event.event === "call_exit" ? [[event.label, event.duration_ns] as const] : [],
      ),
    );
    const summary = buildDataPaths(counterEvents).find((path) => path.name === "summary")!;
    expect(summary.steps.map((step) => step.durationNs)).toEqual([
      0,
      durations.get("increment"),
      durations.get("parityOf"),
      durations.get("summarize"),
    ]);
    expect(summary.totalNs).toBe(
      durations.get("increment")! + durations.get("parityOf")! + durations.get("summarize")!,
    );
  });

  it("counts nested calls once and lists inputs off the chain", () => {
    const location = { file: "x.ts", line: 1, column: 1 };
    const call = { site_id: "s", roots: ["a"], ...location };
    const events: TraceEvent[] = [
      { event: "root", at_ns, value_id: 1, binding: "a", value_preview: "1", ...location },
      { event: "root", at_ns, value_id: 2, binding: "b", value_preview: "2", ...location },
      { event: "call_enter", at_ns, call_id: 1, label: "outer", dependencies: ["a"], ...call },
      {
        event: "call_enter",
        at_ns,
        call_id: 2,
        parent_call_id: 1,
        label: "inner",
        dependencies: ["a"],
        ...call,
      },
      {
        event: "call_exit",
        at_ns,
        call_id: 2,
        label: "inner",
        roots: ["a"],
        duration_ns: 5,
        unwind: false,
      },
      {
        event: "call_exit",
        at_ns,
        call_id: 1,
        label: "outer",
        roots: ["a"],
        duration_ns: 20,
        unwind: false,
      },
      {
        event: "value_derive",
        at_ns,
        value_id: 3,
        label: "x",
        dependencies: ["a", "b"],
        producer_call_ids: [2, 1],
        value_preview: "3",
        roots: ["a", "b"],
        ...location,
      },
      { event: "call_enter", at_ns, call_id: 3, label: "log", dependencies: ["x"], ...call },
      {
        event: "call_exit",
        at_ns,
        call_id: 3,
        label: "log",
        roots: ["a"],
        duration_ns: 7,
        result_preview: "undefined",
        unwind: true,
      },
    ];

    const [, , x, log] = buildDataPaths(events);
    expect(x.steps.map((step) => step.name)).toEqual(["a", "x"]);
    expect(x.steps[1].calls.map((c) => c.label)).toEqual(["inner()", "outer()"]);
    expect(x.totalNs).toBe(20);
    expect(x.otherInputs).toEqual(["b"]);

    // An unbound call result still gets a row on top of its inputs' path.
    expect(log).toMatchObject({ name: "log()", totalNs: 27, threw: true });
    expect(log.steps.map((step) => step.name)).toEqual(["a", "x", "log()"]);
  });
});

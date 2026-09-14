import path from "node:path";
import { fileURLToPath } from "node:url";
import { createServer } from "vite";
import type { ViteDevServer } from "vite";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import runes, { RUNTIME_VIRTUAL_ID } from "../src/core/unplugin.ts";
import type { TraceEvent } from "../src/core/runtime.ts";

const ROOT = fileURLToPath(new URL("..", import.meta.url));

describe("unplugin-runes", () => {
  let server: ViteDevServer;
  let events: TraceEvent[];

  beforeAll(async () => {
    server = await createServer({
      root: ROOT,
      logLevel: "silent",
      server: { middlewareMode: true },
      plugins: [runes.vite()],
    });
    // Loading the demo through the dev server runs the full pipeline:
    // text downgrade -> AST instrumentation -> virtual runtime module.
    await server.ssrLoadModule("/examples/demo.ts");
    const runtime = (await server.ssrLoadModule(RUNTIME_VIRTUAL_ID)) as {
      takeTraceEvents: () => TraceEvent[];
    };
    events = runtime.takeTraceEvents();
  });

  afterAll(async () => {
    await server.close();
  });

  it("records root declarations with original positions", () => {
    const roots = events.filter((e) => e.event === "root");
    const bindings = roots.map((e) => (e.event === "root" ? e.binding : ""));
    expect(bindings).toContain("count");
    expect(bindings).toContain("user");
    const count = roots.find((e) => e.event === "root" && e.binding === "count");
    expect(count).toMatchObject({ file: "examples/demo.ts", line: 30, column: 1 });
  });

  it("records derived values", () => {
    const labels = events
      .filter((e) => e.event === "value_derive")
      .map((e) => (e.event === "value_derive" ? e.label : ""));
    for (const label of ["doubled", "upper", "total", "description", "bumped", "combined"]) {
      expect(labels).toContain(label);
    }
  });

  it("pairs call enter/exit events with merged roots", () => {
    const enters = events.filter((e) => e.event === "call_enter");
    const exits = events.filter((e) => e.event === "call_exit");
    expect(enters.length).toBe(exits.length);

    const double = enters.find((e) => e.event === "call_enter" && e.label === "double");
    expect(double).toMatchObject({ roots: ["count"] });

    // combine(upper, doubled) merges the user and count provenance chains.
    const combine = enters.find((e) => e.event === "call_enter" && e.label === "combine");
    expect(combine).toMatchObject({ roots: ["count", "user"] });
  });

  it("serves the runtime as JavaScript from the virtual module", async () => {
    const resolved = await server.pluginContainer.resolveId(RUNTIME_VIRTUAL_ID);
    expect(resolved?.id).toBe("\0" + RUNTIME_VIRTUAL_ID);
    const loaded = await server.pluginContainer.load(resolved!.id);
    const code = typeof loaded === "object" && loaded ? loaded.code : "";
    // Transpiled output: no TypeScript-only syntax left behind.
    expect(code).toContain("export function markRoot");
    expect(code).not.toContain("interface ");
  });

  it("skips files that never use the keyword", () => {
    // Call the raw hooks directly: no module-graph involvement needed.
    const raw = runes.raw({}, { framework: "vite" });
    const id = path.join(ROOT, "src/plain.ts");
    expect(raw.transformInclude?.(id)).toBe(true);
    const transform = raw.transform;
    expect(typeof transform).toBe("function");
    if (typeof transform !== "function") return;
    const result = transform.call({} as never, "export const answer: number = 42;\n", id);
    expect(result).toBeNull();
  });

  it("skips node_modules and non-TypeScript files", () => {
    const raw = runes.raw({}, { framework: "vite" });
    expect(raw.transformInclude?.("/x/node_modules/pkg/index.ts")).toBe(false);
    expect(raw.transformInclude?.("/x/src/style.css")).toBe(false);
  });
});

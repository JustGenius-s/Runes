import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { afterAll, describe, expect, it } from "vitest";
import { instrumentSource, transpileToJs } from "../src/core/transform.ts";

const RUNTIME_SOURCE_PATH = new URL("../src/core/runtime.ts", import.meta.url);

const OPTIONS = {
  fileName: "demo.ts",
  runtimeSpecifier: "./runes.runtime.js",
};

describe("instrumentSource", () => {
  it("injects the runtime import", () => {
    const out = instrumentSource("rune x = 1;", OPTIONS);
    expect(out).toContain('import * as __runes from "./runes.runtime.js";');
  });

  it("wraps dependent calls and derived bindings", () => {
    const out = instrumentSource(
      [
        "function double(n: number) { return n * 2; }",
        "rune count = 21;",
        "const doubled = double(count);",
      ].join("\n"),
      OPTIONS,
    );
    expect(out).toContain('__runes.call("double", ["count"], ["count"]');
    expect(out).toContain('__runes.derive("doubled", __runes.call("double"');
  });

  it("preserves await in derived initializers", () => {
    const out = instrumentSource(
      [
        "async function load() {",
        "  rune count = 21;",
        "  const resolved = await Promise.resolve(count);",
        "  return resolved;",
        "}",
      ].join("\n"),
      OPTIONS,
    );
    expect(out).toContain('__runes.derive("resolved", await __runes.call');
  });

  it("leaves independent calls untouched", () => {
    const out = instrumentSource(
      ["function rand() { return 1; }", "const r = rand();"].join("\n"),
      OPTIONS,
    );
    expect(out).not.toContain("__runes.call(");
  });

  it("tracks closures capturing a root", () => {
    const out = instrumentSource(
      ["rune count = 1;", "const bump = (n: number) => n + count;"].join("\n"),
      OPTIONS,
    );
    expect(out).toContain('__runes.derive("bump"');
  });
});

describe("end to end", () => {
  const dir = mkdtempSync(path.join(tmpdir(), "runes-ts-"));
  afterAll(() => rmSync(dir, { recursive: true, force: true }));

  it("produces paired schema v4 events with direct dependencies and values", () => {
    const source = [
      "function double(n: number) { return n * 2; }",
      'function boom(): never { throw new Error("x"); }',
      "rune count = 21;",
      "const doubled = double(count);",
      "try { boom(); } catch {",
      "  // count is not involved: no call event for boom",
      "}",
      "console.log(doubled);",
    ].join("\n");
    writeFileSync(
      path.join(dir, "runes.runtime.js"),
      transpileToJs(readFileSync(RUNTIME_SOURCE_PATH, "utf8"), "runes.runtime.ts"),
    );
    writeFileSync(
      path.join(dir, "demo.js"),
      transpileToJs(instrumentSource(source, OPTIONS), "demo.ts"),
    );
    const tracePath = path.join(dir, "trace.json");
    const run = spawnSync(process.execPath, [path.join(dir, "demo.js")], {
      env: { ...process.env, RUNES_OUT: tracePath, RUNES_PRINT: "0" },
    });
    expect(run.status).toBe(0);

    const trace = JSON.parse(readFileSync(tracePath, "utf8"));
    expect(trace.schema_version).toBe(4);
    expect(trace.overflowed_events).toBe(0);

    const events = trace.events as Array<Record<string, unknown>>;
    const root = events.find((e) => e.event === "root");
    expect(root).toMatchObject({ binding: "count", value_preview: "21", file: "demo.ts" });

    const enter = events.find((e) => e.event === "call_enter" && e.label === "double");
    expect(enter).toMatchObject({ roots: ["count"], dependencies: ["count"] });
    const exit = events.find((e) => e.event === "call_exit" && e.call_id === enter?.call_id);
    expect(exit).toMatchObject({ label: "double", result_preview: "42", unwind: false });
    expect(typeof exit?.duration_ns).toBe("number");

    const derive = events.find((e) => e.event === "value_derive" && e.label === "doubled");
    expect(derive).toMatchObject({
      roots: ["count"],
      dependencies: ["count"],
      producer_call_ids: [enter?.call_id],
      value_preview: "42",
    });

    // boom() does not reference count, so it stays uninstrumented.
    expect(events.some((e) => e.event === "call_enter" && e.label === "boom")).toBe(false);
  });
});

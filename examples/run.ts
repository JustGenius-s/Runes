// Run the demo through the unplugin pipeline: a Vite dev server in
// middleware mode instruments the module graph in memory and serves the
// runtime as a virtual module. RUNES_OUT is set before any module loads,
// so the runtime writes the trace on exit.
import { mkdirSync } from "node:fs";
import { createServer } from "vite";
import runes from "../src/core/unplugin.ts";

process.env.RUNES_OUT ??= ".runes/trace.json";
mkdirSync(".runes", { recursive: true });

const server = await createServer({
  logLevel: "silent",
  server: { middlewareMode: true },
  plugins: [runes.vite()],
});
try {
  await server.ssrLoadModule("/examples/demo.ts");
} finally {
  await server.close();
}
console.log("[runes] trace written to " + process.env.RUNES_OUT);

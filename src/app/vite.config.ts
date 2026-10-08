import path from "node:path";
import { fileURLToPath } from "node:url";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";
import { runes } from "../core/index.ts";

const appRoot = path.dirname(fileURLToPath(import.meta.url));

// Browsers coarsen performance.now() to ~100 µs unless the page is
// cross-origin isolated; isolation gives the trace durations ~5 µs resolution.
const crossOriginIsolation = {
  "Cross-Origin-Opener-Policy": "same-origin",
  "Cross-Origin-Embedder-Policy": "require-corp",
};

export default defineConfig({
  // The dependency scanner parses application modules before Vite plugins run.
  // Disable discovery so `rune` declarations are first lowered by the Runes
  // transform instead of being parsed as plain TypeScript by the scanner.
  optimizeDeps: {
    noDiscovery: true,
    include: [
      "bits-ui",
      "highlight.js/lib/core",
      "highlight.js/lib/languages/typescript",
    ],
  },
  resolve: {
    alias: {
      $lib: path.join(appRoot, "lib"),
    },
  },
  plugins: [
    // Instrument only experiment scenario files (they use the rune keyword).
    runes.vite({ include: (id) => id.includes("/experiments/") }),
    tailwindcss(),
    svelte(),
  ],
  server: {
    port: 5170,
    headers: crossOriginIsolation,
  },
  preview: {
    port: 5170,
    headers: crossOriginIsolation,
  },
  build: {
    outDir: path.join(appRoot, "..", "..", "dist-app"),
    emptyOutDir: true,
  },
});

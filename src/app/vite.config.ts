import path from "node:path";
import { fileURLToPath } from "node:url";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";
import { runes } from "../core/index.ts";

const appRoot = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  // The dependency scanner parses application modules before Vite plugins run.
  // Disable discovery so `rune` declarations are first lowered by the Runes
  // transform instead of being parsed as plain TypeScript by the scanner.
  optimizeDeps: {
    noDiscovery: true,
    include: [
      "@svar-ui/svelte-gantt",
      "@xyflow/svelte",
      "bits-ui",
      "highlight.js/lib/core",
      "highlight.js/lib/languages/typescript",
      "layerchart",
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
  },
  preview: {
    port: 5170,
  },
  build: {
    outDir: path.join(appRoot, "..", "..", "dist-app"),
    emptyOutDir: true,
  },
});

import path from "node:path";
import { fileURLToPath } from "node:url";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";
import { runes } from "../core/index.ts";

const appRoot = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
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

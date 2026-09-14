import { defineConfig } from "vite-plus";

export default defineConfig({
  pack: {
    entry: ["src/core/index.ts"],
    deps: { resolveDepSubpath: true },
    dts: {
      generator: "tsgo",
    },
    exports: true,
  },
  lint: {
    ignorePatterns: ["dist/**", "dist-app/**", ".runes/**", "examples/**", "src/app/**"],
    options: {
      typeAware: true,
      typeCheck: true,
    },
  },
  fmt: {
    ignorePatterns: [".runes/**", "examples/**", "src/app/**"],
  },
});

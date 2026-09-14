/**
 * Bundler plugin entry: run the Runes downgrade + instrumentation inside
 * Vite, Rollup, esbuild or webpack.
 *
 * The plugin hooks the bundler's transform pipeline, where the raw source
 * is still available before any parser runs, so the \`rune\` keyword can be
 * lowered before it is parsed. The runtime is served as a virtual module
 * (virtual:runes-runtime), so no runtime file is emitted and no import
 * paths need to be computed.
 */
import { readFileSync } from "node:fs";
import path from "node:path";
import { createUnplugin } from "unplugin";
import type { UnpluginFactory } from "unplugin";
import { instrumentSource, transpileToJs } from "./transform.ts";

/** Import specifier instrumented files use to reach the runtime. */
export const RUNTIME_VIRTUAL_ID = "virtual:runes-runtime";

const RESOLVED_RUNTIME_ID = "\0" + RUNTIME_VIRTUAL_ID;

/** Options accepted by the Runes bundler plugin. */
export interface RunesPluginOptions {
  /** Root declaration keyword. Defaults to 'rune'. */
  keyword?: string;
  /** Extra filter for modules to instrument. node_modules is always skipped. */
  include?: (id: string) => boolean;
}

let runtimeSourceCache: string | undefined;

function runtimeSource(): string {
  runtimeSourceCache ??= readFileSync(new URL("./runtime.ts", import.meta.url), "utf8");
  return runtimeSourceCache;
}

export const runesPluginFactory: UnpluginFactory<RunesPluginOptions | undefined, false> = (
  options = {},
) => {
  const keyword = options.keyword ?? "rune";
  return {
    name: "unplugin-runes",
    // Must run before the host's own TS transform: the rune keyword is not
    // valid TypeScript and has to be lowered before any parser sees it.
    enforce: "pre",
    resolveId(id) {
      if (id === RUNTIME_VIRTUAL_ID) return RESOLVED_RUNTIME_ID;
      return null;
    },
    load(id) {
      if (id !== RESOLVED_RUNTIME_ID) return null;
      // Transpile here so the virtual module works no matter how the host
      // bundler would classify a bare virtual id.
      return { code: transpileToJs(runtimeSource(), "runes.runtime"), map: null };
    },
    transformInclude(id) {
      if (id.includes("node_modules")) return false;
      if (!/\.tsx?$/.test(id)) return false;
      return options.include ? options.include(id) : true;
    },
    transform(code, id) {
      // Files without the keyword cannot declare roots, and per-file
      // instrumentation would only inject an unused runtime import.
      if (!code.includes(keyword)) return null;
      const fileName = path.relative(process.cwd(), id) || id;
      const instrumented = instrumentSource(code, {
        fileName,
        runtimeSpecifier: RUNTIME_VIRTUAL_ID,
        keyword,
      });
      return { code: instrumented, map: null };
    },
  };
};

/**
 * The Runes plugin for every supported bundler. Use the per-bundler
 * properties, e.g. \`runes.vite()\` in a Vite config.
 */
export const runes = createUnplugin(runesPluginFactory);

export default runes;

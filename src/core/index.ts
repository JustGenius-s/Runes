/**
 * runes-ts: mark a plain TypeScript value with 'rune' and the compiler
 * traces its local derivations and dependent calls at runtime.
 */
export { default, runes, RUNTIME_VIRTUAL_ID } from "./unplugin.ts";
export type { RunesPluginOptions } from "./unplugin.ts";

/**
 * Runtime module source, served by the bundler plugin as the virtual
 * module virtual:runes-runtime, so users never install or import a
 * runtime package themselves. Events follow trace schema v4, compatible
 * with the Runes viewer.
 *
 * The runtime is dual-environment: under Node.js it uses hrtime and can
 * write trace files; in the browser it falls back to performance.now()
 * and keeps events in memory for takeTraceEvents/subscribeTrace.
 */

interface BaseEvent {
  at_ns: number;
}

export interface RootEvent extends BaseEvent {
  event: "root";
  value_id: number;
  binding: string;
  value_preview: string;
  file: string;
  line: number;
  column: number;
}

export interface CallEnterEvent extends BaseEvent {
  event: "call_enter";
  call_id: number;
  site_id: string;
  label: string;
  dependencies: string[];
  parent_call_id?: number;
  file: string;
  line: number;
  column: number;
  roots: string[];
}

export interface CallExitEvent extends BaseEvent {
  event: "call_exit";
  call_id: number;
  label: string;
  roots: string[];
  duration_ns: number;
  result_preview?: string;
  unwind: boolean;
}

export interface ValueDeriveEvent extends BaseEvent {
  event: "value_derive";
  value_id: number;
  label: string;
  dependencies: string[];
  producer_call_ids: number[];
  value_preview: string;
  file: string;
  line: number;
  column: number;
  roots: string[];
}

export type TraceEvent = RootEvent | CallEnterEvent | CallExitEvent | ValueDeriveEvent;

/** Listener invoked synchronously for every recorded event. */
export type TraceListener = (event: TraceEvent) => void;

const isNode = typeof process !== "undefined" && typeof process.versions?.node === "string";

const nodeStartNs = isNode ? process.hrtime.bigint() : 0n;
const browserStartMs = isNode ? 0 : performance.now();
const capacity = isNode ? Number(process.env.RUNES_BUFFER_CAPACITY ?? 4096) : 65536;
// Printing every event is useful for CLI runs but noisy in devtools, so
// it stays opt-in outside Node.
const shouldPrint = isNode ? process.env.RUNES_PRINT !== "0" : false;

const events: TraceEvent[] = [];
const listeners = new Set<TraceListener>();
let overflowed = 0;
let callSequence = 0;
let valueSequence = 0;
interface TraceContext {
  roots: string[];
  callId?: number;
}

const contextStack: TraceContext[] = [];
const completedCallsBySite = new Map<string, number[]>();

function nowNs(): number {
  if (isNode) return Number(process.hrtime.bigint() - nodeStartNs);
  return Math.round((performance.now() - browserStartMs) * 1e6);
}

function formatEvent(event: TraceEvent): string {
  const at = "+" + event.at_ns + "ns";
  switch (event.event) {
    case "root":
      return (
        "[Runes " +
        at +
        "] root       " +
        event.binding +
        " @ " +
        event.file +
        ":" +
        event.line +
        ":" +
        event.column
      );
    case "call_enter":
      return (
        "[Runes " +
        at +
        "] call.enter #" +
        event.call_id +
        " " +
        event.label +
        " roots=[" +
        event.roots.join(", ") +
        "]" +
        " @ " +
        event.file +
        ":" +
        event.line +
        ":" +
        event.column
      );
    case "call_exit":
      return (
        "[Runes " +
        at +
        "] call.exit  #" +
        event.call_id +
        " " +
        event.label +
        " roots=[" +
        event.roots.join(", ") +
        "]" +
        " duration=" +
        event.duration_ns +
        "ns" +
        (event.unwind ? " (unwind)" : "")
      );
    case "value_derive":
      return (
        "[Runes " +
        at +
        "] value.derive " +
        event.label +
        " roots=[" +
        event.roots.join(", ") +
        "]" +
        " @ " +
        event.file +
        ":" +
        event.line +
        ":" +
        event.column
      );
  }
}

function push(event: TraceEvent): void {
  if (events.length < capacity) {
    events.push(event);
  } else {
    overflowed++;
  }
  if (shouldPrint) console.log(formatEvent(event));
  for (const listener of listeners) listener(event);
}

/** Roots of the innermost instrumented call, for manual propagation. */
export function currentRoots(): string[] {
  return contextStack.length > 0 ? contextStack[contextStack.length - 1].roots.slice() : [];
}

function currentCallId(): number | undefined {
  for (let i = contextStack.length - 1; i >= 0; i--) {
    if (contextStack[i].callId !== undefined) return contextStack[i].callId;
  }
  return undefined;
}

/** Runs f with roots as the active trace context. */
export function withRoots<T>(roots: string[], f: () => T): T {
  contextStack.push({ roots });
  try {
    return f();
  } finally {
    contextStack.pop();
  }
}

/** Records a root declaration and returns the value unchanged. */
export function markRoot<T>(name: string, value: T, file: string, line: number, column: number): T {
  completedCallsBySite.clear();
  valueSequence++;
  push({
    event: "root",
    at_ns: nowNs(),
    value_id: valueSequence,
    binding: name,
    value_preview: previewValue(value),
    file,
    line,
    column,
  });
  return value;
}

/** Records a value derived from tracked roots, returning it unchanged. */
export function derive<T>(
  label: string,
  value: T,
  roots: string[],
  dependencies: string[],
  producerSites: string[],
  file: string,
  line: number,
  column: number,
): T {
  const producerCallIds: number[] = [];
  for (const site of producerSites) {
    producerCallIds.push(...(completedCallsBySite.get(site) ?? []));
    completedCallsBySite.delete(site);
  }
  valueSequence++;
  push({
    event: "value_derive",
    at_ns: nowNs(),
    value_id: valueSequence,
    label,
    dependencies,
    producer_call_ids: producerCallIds,
    value_preview: previewValue(value),
    file,
    line,
    column,
    roots,
  });
  return value;
}

function mergeRoots(roots: string[]): string[] {
  const merged = new Set(roots);
  for (const root of currentRoots()) merged.add(root);
  return [...merged].sort();
}

function recordExit(
  callId: number,
  label: string,
  roots: string[],
  startedNs: number,
  unwind: boolean,
  siteId: string,
  result?: unknown,
): void {
  push({
    event: "call_exit",
    at_ns: nowNs(),
    call_id: callId,
    label,
    roots,
    duration_ns: nowNs() - startedNs,
    ...(unwind ? {} : { result_preview: previewValue(result) }),
    unwind,
  });
  const completed = completedCallsBySite.get(siteId) ?? [];
  completed.push(callId);
  completedCallsBySite.set(siteId, completed);
}

function previewValue(value: unknown): string {
  let text: string;
  try {
    if (typeof value === "string") text = JSON.stringify(value);
    else if (typeof value === "undefined") text = "undefined";
    else if (typeof value === "bigint") text = `${value}n`;
    else if (typeof value === "symbol")
      text = value.description ? `Symbol(${value.description})` : "Symbol()";
    else if (typeof value === "function") text = `[Function ${value.name || "anonymous"}]`;
    else text = JSON.stringify(value) ?? Object.prototype.toString.call(value);
  } catch {
    text = Object.prototype.toString.call(value);
  }
  return text.length > 120 ? `${text.slice(0, 117)}…` : text;
}

function isThenable(value: unknown): value is PromiseLike<unknown> {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof (value as { then?: unknown }).then === "function"
  );
}

/**
 * Runs thunk while recording call_enter/call_exit. Promise results are
 * measured until settlement; sync exceptions and rejections are recorded
 * as unwind exits so events always pair up.
 */
export function call<T>(
  label: string,
  roots: string[],
  dependencies: string[],
  siteId: string,
  thunk: () => T,
  file: string,
  line: number,
  column: number,
): T {
  callSequence++;
  const callId = callSequence;
  const merged = mergeRoots(roots);
  const parentCallId = currentCallId();
  push({
    event: "call_enter",
    at_ns: nowNs(),
    call_id: callId,
    site_id: siteId,
    label,
    dependencies,
    ...(parentCallId === undefined ? {} : { parent_call_id: parentCallId }),
    file,
    line,
    column,
    roots: merged,
  });
  const startedNs = nowNs();
  contextStack.push({ roots: merged, callId });
  try {
    const result = thunk();
    if (isThenable(result)) {
      return result.then(
        (value) => {
          contextStack.pop();
          recordExit(callId, label, merged, startedNs, false, siteId, value);
          return value;
        },
        (error: unknown) => {
          contextStack.pop();
          recordExit(callId, label, merged, startedNs, true, siteId);
          throw error;
        },
      ) as T;
    }
    contextStack.pop();
    recordExit(callId, label, merged, startedNs, false, siteId, result);
    return result;
  } catch (error) {
    contextStack.pop();
    recordExit(callId, label, merged, startedNs, true, siteId);
    throw error;
  }
}

/** Returns a copy of all recorded events. */
export function takeTraceEvents(): TraceEvent[] {
  return events.slice();
}

/** Drops all recorded events; call/value sequences keep increasing. */
export function clearTraceEvents(): void {
  events.length = 0;
  overflowed = 0;
  completedCallsBySite.clear();
}

/**
 * Subscribes to trace events as they are recorded. Returns an
 * unsubscribe function. Useful for live trace views in the browser.
 */
export function subscribeTrace(listener: TraceListener): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/** Number of events dropped because the buffer reached capacity. */
export function overflowedEventCount(): number {
  return overflowed;
}

/** Writes the trace document (schema v4) to path. Node.js only. */
export function writeTraceFile(path?: string): string {
  if (!isNode) {
    throw new Error("writeTraceFile is only available under Node.js");
  }
  const { writeFileSync } = process.getBuiltinModule("node:fs");
  const target = path ?? process.env.RUNES_OUT ?? "trace.json";
  const sorted = events.slice().sort((a, b) => a.at_ns - b.at_ns);
  const document = { schema_version: 4, overflowed_events: overflowed, events: sorted };
  writeFileSync(target, JSON.stringify(document));
  return target;
}

// Auto-write the trace on exit when RUNES_OUT is set, so business code
// does not need to call any tracing API by hand.
if (isNode && process.env.RUNES_OUT) {
  process.on("exit", () => {
    writeTraceFile(process.env.RUNES_OUT);
  });
}

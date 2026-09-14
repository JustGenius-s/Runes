/**
 * Runtime module source, served by the bundler plugin as the virtual
 * module virtual:runes-runtime, so users never install or import a
 * runtime package themselves. Events follow trace schema v3, compatible
 * with the Runes viewer.
 */
import { writeFileSync } from "node:fs";

interface BaseEvent {
  at_ns: number;
}

export interface RootEvent extends BaseEvent {
  event: "root";
  binding: string;
  file: string;
  line: number;
  column: number;
}

export interface CallEnterEvent extends BaseEvent {
  event: "call_enter";
  call_id: number;
  label: string;
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
  unwind: boolean;
}

export interface ValueDeriveEvent extends BaseEvent {
  event: "value_derive";
  value_id: number;
  label: string;
  file: string;
  line: number;
  column: number;
  roots: string[];
}

export type TraceEvent = RootEvent | CallEnterEvent | CallExitEvent | ValueDeriveEvent;

const startTime = process.hrtime.bigint();
const capacity = Number(process.env.RUNES_BUFFER_CAPACITY ?? 4096);
const shouldPrint = process.env.RUNES_PRINT !== "0";

const events: TraceEvent[] = [];
let overflowed = 0;
let callSequence = 0;
let valueSequence = 0;
const contextStack: string[][] = [];

function nowNs(): number {
  return Number(process.hrtime.bigint() - startTime);
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
}

/** Roots of the innermost instrumented call, for manual propagation. */
export function currentRoots(): string[] {
  return contextStack.length > 0 ? contextStack[contextStack.length - 1].slice() : [];
}

/** Runs f with roots as the active trace context. */
export function withRoots<T>(roots: string[], f: () => T): T {
  contextStack.push(roots);
  try {
    return f();
  } finally {
    contextStack.pop();
  }
}

/** Records a root declaration and returns the value unchanged. */
export function markRoot<T>(name: string, value: T, file: string, line: number, column: number): T {
  push({ event: "root", at_ns: nowNs(), binding: name, file, line, column });
  return value;
}

/** Records a value derived from tracked roots, returning it unchanged. */
export function derive<T>(
  label: string,
  value: T,
  roots: string[],
  file: string,
  line: number,
  column: number,
): T {
  valueSequence++;
  push({
    event: "value_derive",
    at_ns: nowNs(),
    value_id: valueSequence,
    label,
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
): void {
  push({
    event: "call_exit",
    at_ns: nowNs(),
    call_id: callId,
    label,
    roots,
    duration_ns: nowNs() - startedNs,
    unwind,
  });
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
  thunk: () => T,
  file: string,
  line: number,
  column: number,
): T {
  callSequence++;
  const callId = callSequence;
  const merged = mergeRoots(roots);
  push({
    event: "call_enter",
    at_ns: nowNs(),
    call_id: callId,
    label,
    file,
    line,
    column,
    roots: merged,
  });
  const startedNs = nowNs();
  contextStack.push(merged);
  try {
    const result = thunk();
    if (isThenable(result)) {
      return result.then(
        (value) => {
          contextStack.pop();
          recordExit(callId, label, merged, startedNs, false);
          return value;
        },
        (error: unknown) => {
          contextStack.pop();
          recordExit(callId, label, merged, startedNs, true);
          throw error;
        },
      ) as T;
    }
    contextStack.pop();
    recordExit(callId, label, merged, startedNs, false);
    return result;
  } catch (error) {
    contextStack.pop();
    recordExit(callId, label, merged, startedNs, true);
    throw error;
  }
}

/** Returns a copy of all recorded events. */
export function takeTraceEvents(): TraceEvent[] {
  return events.slice();
}

/** Number of events dropped because the buffer reached capacity. */
export function overflowedEventCount(): number {
  return overflowed;
}

/** Writes the trace document (schema v3) to path. */
export function writeTraceFile(path?: string): string {
  const target = path ?? process.env.RUNES_OUT ?? "trace.json";
  const sorted = events.slice().sort((a, b) => a.at_ns - b.at_ns);
  const document = { schema_version: 3, overflowed_events: overflowed, events: sorted };
  writeFileSync(target, JSON.stringify(document));
  return target;
}

// Auto-write the trace on exit when RUNES_OUT is set, so business code
// does not need to call any tracing API by hand.
if (process.env.RUNES_OUT) {
  process.on("exit", () => {
    writeTraceFile(process.env.RUNES_OUT);
  });
}

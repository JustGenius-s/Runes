import type { ITask } from "@svar-ui/svelte-gantt";
import type { TraceEvent } from "virtual:runes-runtime";

interface Span {
	id: string;
	kind: "call" | "root" | "derive";
	label: string;
	start: number;
	end: number;
	startOrder: number;
	endOrder: number;
	startStep: number;
	endStep: number;
	callTime?: number;
	pending: boolean;
	unwind: boolean;
}

export interface TraceGanttTask extends ITask {
	id: string;
	count: number;
	timing: string;
	traceStart: number;
	traceEnd: number;
	pendingCount: number;
	errorCount: number;
	group: boolean;
}

// SVAR uses dates for positioning. One calendar day represents one execution
// step; real nanosecond readings are kept separately in task metadata.
export function stepDate(step: number): Date {
	return new Date(2000, 0, 1 + step);
}

export function formatStep(date: Date): string {
	return String(Math.round(
		(Date.UTC(date.getFullYear(), date.getMonth(), date.getDate()) - Date.UTC(2000, 0, 1)) / 86_400_000,
	));
}

export function buildTraceGantt(events: TraceEvent[], expanded: Record<string, boolean>) {
	const spans: Span[] = [];
	const open = new Map<number, Span>();
	const origin = events.reduce((min, event) => Math.min(min, event.at_ns), Infinity);
	const latest = events.reduce((max, event) => Math.max(max, event.at_ns), 0);
	let step = 0;

	for (const [order, event] of events.entries()) {
		if (event.event === "call_exit") {
			const span = event.call_id === undefined ? undefined : open.get(event.call_id);
			if (!span) continue;
			span.end = event.at_ns - origin;
			span.endOrder = order;
			span.endStep = ++step;
			span.callTime = event.duration_ns;
			span.pending = false;
			span.unwind = event.unwind ?? false;
			open.delete(event.call_id!);
			continue;
		}
		if (event.event === "call_enter" && event.call_id === undefined) continue;
		if (event.event === "call_enter" && open.size) step++;
		const span: Span = {
			id: event.event === "call_enter" ? `call-${event.call_id}` : `event-${order}`,
			kind: event.event === "call_enter" ? "call" : event.event === "root" ? "root" : "derive",
			label: event.event === "root" ? event.binding ?? "?" : event.label ?? "?",
			start: event.at_ns - origin,
			end: event.at_ns - origin,
			startOrder: order,
			endOrder: order,
			startStep: step,
			endStep: step,
			pending: event.event === "call_enter",
			unwind: false,
		};
		spans.push(span);
		if (event.event === "call_enter") open.set(event.call_id!, span);
	}
	if (open.size) step++;
	for (const span of open.values()) {
		span.end = latest - origin;
		span.endOrder = events.length - 1;
		span.endStep = step;
	}

	// Calls are the only timed operations. Root and derive events belong in
	// the dependency flow and raw event views, where their meaning is clearer.
	const callSpans = spans.filter((span) => span.kind === "call");
	const groups = new Map<string, Span[]>();
	for (const span of callSpans) {
		const key = `group-${span.kind}-${span.label}`;
		const group = groups.get(key) ?? [];
		group.push(span);
		groups.set(key, group);
	}

	const tasks: TraceGanttTask[] = [];
	for (const [groupId, group] of groups) {
		const first = group[0];
		const label = first.label + (first.kind === "call" ? "()" : "");
		if (group.length > 1) {
			tasks.push({
				id: groupId,
				text: label,
				type: "summary",
				open: expanded[groupId] ?? false,
				start: stepDate(Math.min(...group.map((span) => span.startStep))),
				end: stepDate(Math.max(...group.map((span) => span.endStep))),
				count: group.length,
				timing: first.kind === "call" ? formatMeasuredNs(group.reduce((sum, span) => sum + (span.callTime ?? span.end - span.start), 0)) : "—",
				traceStart: Math.min(...group.map((span) => span.start)),
				traceEnd: Math.max(...group.map((span) => span.end)),
				pendingCount: group.filter((span) => span.pending).length,
				errorCount: group.filter((span) => span.unwind).length,
				group: true,
			});
		}
		for (const [index, span] of group.entries()) {
			tasks.push({
				id: span.id,
				parent: group.length > 1 ? groupId : 0,
				text: group.length > 1 ? `${label} #${index + 1}` : label,
				type: span.kind === "call" ? "task" : "milestone",
				start: stepDate(span.startStep),
				end: stepDate(span.endStep),
				count: 1,
				timing: span.kind === "call" ? (span.pending ? "pending" : formatMeasuredNs(span.callTime ?? span.end - span.start)) : "—",
				traceStart: span.start,
				traceEnd: span.end,
				pendingCount: Number(span.pending),
				errorCount: Number(span.unwind),
				group: false,
			});
		}
	}

	return {
		tasks,
		links: [],
		calls: callSpans.length,
		functions: [...groups.values()].filter((group) => group[0].kind === "call").length,
		duration: events.length ? latest - origin : 0,
		steps: Math.max(1, step),
	};
}

export function formatNs(ns: number): string {
	if (ns >= 1e9) return `${(ns / 1e9).toFixed(2)} s`;
	if (ns >= 1e6) return `${(ns / 1e6).toFixed(2)} ms`;
	if (ns >= 1000) return `${(ns / 1000).toFixed(1)} µs`;
	return `${Math.round(ns)} ns`;
}

function formatMeasuredNs(ns: number): string {
	return ns === 0 ? "< resolution" : formatNs(ns);
}

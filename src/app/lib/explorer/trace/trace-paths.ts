import type { TraceEvent } from "virtual:runes-runtime";

/** A traced call that produced one step of a data path. */
export interface PathCall {
	callId: number;
	parentCallId?: number;
	label: string;
	/** Undefined while the call has not returned. */
	durationNs?: number;
	threw: boolean;
}

/** One value on a path, with the calls that produced it from the previous step. */
export interface PathStep {
	id: string;
	name: string;
	calls: PathCall[];
	/** Time spent producing this step; nested calls are counted once, via their parent. */
	durationNs: number;
}

/** A traced value and the dependency chain that leads to it from an input. */
export interface DataPath {
	id: string;
	name: string;
	value?: string;
	input: boolean;
	/** Input first, this value last. */
	steps: PathStep[];
	/** Direct inputs that feed this value but are not on the displayed chain. */
	otherInputs: string[];
	/** Sum of step durations along the chain. */
	totalNs: number;
	pending: boolean;
	threw: boolean;
}

interface CallRecord {
	call: PathCall;
	sources: string[];
	order: number;
	result?: string;
	consumed: boolean;
}

/**
 * Builds one path per traced value, in execution order. A value with several
 * inputs follows the longest (then slowest) chain, so every intermediate
 * value it waited on is shown; inputs off that chain are listed separately.
 */
export function buildDataPaths(events: TraceEvent[]): DataPath[] {
	const exits = new Map<number, TraceEvent>();
	for (const event of events) {
		if (event.event === "call_exit" && event.call_id !== undefined) exits.set(event.call_id, event);
	}

	const paths = new Map<string, { path: DataPath; order: number }>();
	const latestByBinding = new Map<string, string>();
	const calls = new Map<number, CallRecord>();

	function resolve(names: string[] | undefined): string[] {
		return (names ?? []).flatMap((name) => latestByBinding.get(name) ?? []);
	}

	function add(id: string, name: string, value: string | undefined, input: boolean, ownCalls: PathCall[], sourceIds: string[], order: number) {
		const sources = [...new Set(sourceIds)].map((sourceId) => paths.get(sourceId)!.path);
		const chain = sources.reduce<DataPath | undefined>(
			(best, source) =>
				!best ||
				source.steps.length > best.steps.length ||
				(source.steps.length === best.steps.length && source.totalNs > best.totalNs)
					? source
					: best,
			undefined,
		);
		const ownIds = new Set(ownCalls.map((call) => call.callId));
		const durationNs = ownCalls
			.filter((call) => call.parentCallId === undefined || !ownIds.has(call.parentCallId))
			.reduce((sum, call) => sum + (call.durationNs ?? 0), 0);
		const steps = [...(chain?.steps ?? []), { id, name, calls: ownCalls, durationNs }];
		const onChain = new Set(steps.map((step) => step.id));
		paths.set(id, {
			order,
			path: {
				id,
				name,
				value,
				input,
				steps,
				otherInputs: sources.filter((source) => !onChain.has(source.id)).map((source) => source.name),
				totalNs: (chain?.totalNs ?? 0) + durationNs,
				pending: (chain?.pending ?? false) || ownCalls.some((call) => call.durationNs === undefined),
				threw: (chain?.threw ?? false) || ownCalls.some((call) => call.threw),
			},
		});
	}

	for (const [order, event] of events.entries()) {
		if (event.event === "root" && event.binding) {
			const id = `value-${event.value_id ?? order}`;
			add(id, event.binding, event.value_preview, true, [], [], order);
			latestByBinding.set(event.binding, id);
		} else if (event.event === "call_enter" && event.call_id !== undefined) {
			const exit = exits.get(event.call_id);
			calls.set(event.call_id, {
				call: {
					callId: event.call_id,
					parentCallId: event.parent_call_id,
					label: `${event.label ?? "unknown"}()`,
					durationNs: exit?.duration_ns,
					threw: exit?.unwind ?? false,
				},
				sources: resolve(event.dependencies),
				order,
				result: exit?.result_preview,
				consumed: false,
			});
		} else if (event.event === "value_derive" && event.label) {
			const producers = (event.producer_call_ids ?? []).flatMap((callId) => calls.get(callId) ?? []);
			for (const producer of producers) producer.consumed = true;
			const id = `value-${event.value_id ?? order}`;
			add(
				id,
				event.label,
				event.value_preview,
				false,
				producers.map((producer) => producer.call),
				[...resolve(event.dependencies), ...producers.flatMap((producer) => producer.sources)],
				order,
			);
			latestByBinding.set(event.label, id);
		}
	}

	// A call whose result is never bound to a name is still data on the path
	// of whatever consumes it, so it gets its own row.
	for (const record of calls.values()) {
		if (record.consumed) continue;
		add(`call-${record.call.callId}`, record.call.label, record.result, false, [record.call], record.sources, record.order);
	}

	return [...paths.values()].sort((a, b) => a.order - b.order).map((entry) => entry.path);
}

/** A data path placed in the dependency tree. */
export interface PathTreeRow {
	path: DataPath;
	depth: number;
	/** Whether this is the last child of its parent. */
	last: boolean;
	hasChildren: boolean;
	/** For each ancestor level above the parent, whether its branch continues below this row. */
	guides: boolean[];
}

/**
 * Nests every path under the previous step of its chain, so walking up the
 * tree from a row retraces that value's path back to its input.
 */
export function layoutPathTree(paths: DataPath[]): PathTreeRow[] {
	const children = new Map<string | undefined, DataPath[]>();
	for (const path of paths) {
		const parent = path.steps.at(-2)?.id;
		children.set(parent, [...(children.get(parent) ?? []), path]);
	}

	const rows: PathTreeRow[] = [];
	function visit(parent: string | undefined, depth: number, guides: boolean[]) {
		const siblings = children.get(parent) ?? [];
		for (const [index, path] of siblings.entries()) {
			const last = index === siblings.length - 1;
			rows.push({ path, depth, last, hasChildren: children.has(path.id), guides });
			visit(path.id, depth + 1, depth === 0 ? [] : [...guides, !last]);
		}
	}
	visit(undefined, 0, []);
	return rows;
}

export function formatNs(ns: number): string {
	if (ns >= 1e9) return `${(ns / 1e9).toFixed(2)} s`;
	if (ns >= 1e6) return `${(ns / 1e6).toFixed(2)} ms`;
	if (ns >= 1000) return `${(ns / 1000).toFixed(1)} µs`;
	return `${Math.round(ns)} ns`;
}

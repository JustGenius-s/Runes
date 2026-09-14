<script lang="ts">
	import MinusIcon from "@lucide/svelte/icons/minus";
	import PlusIcon from "@lucide/svelte/icons/plus";
	import RotateCcwIcon from "@lucide/svelte/icons/rotate-ccw";
	import { Button } from "$lib/components/ui/button/index.js";
	import {
		clearTraceEvents,
		subscribeTrace,
		takeTraceEvents,
		type TraceEvent,
	} from "virtual:runes-runtime";
	import { BarChart, LineChart } from "layerchart";
	import TracePanel from "../../trace/trace-panel.svelte";
	import { step, type CounterStep } from "./scenario.js";

	interface ActionStat {
		action: number;
		calls: number;
		derives: number;
		duration: number;
	}

	const SCENARIO_FILE = "counter/scenario";

	let current = $state(0);
	let result = $state<CounterStep | null>(null);
	let events = $state<TraceEvent[]>([]);
	// Index into the full event buffer where the last action started.
	let lastActionStart = $state(0);
	// Counter value before the last step, shown on the flow graph's root.
	let lastInput = $state<number | null>(null);
	// Per-click stats, one entry per apply() — the Metrics charts' data.
	let history = $state<ActionStat[]>([]);

	$effect(() => {
		events = takeTraceEvents().filter(isScenarioEvent);
		return subscribeTrace((event) => {
			if (isScenarioEvent(event)) events.push(event);
		});
	});

	// call_exit carries no file/line/column (trace schema v3), so it is
	// attributed to this scenario through the call_id of its call_enter.
	const scenarioCallIds = new Set<number>();

	function isScenarioEvent(event: TraceEvent): boolean {
		if (event.event === "call_enter") {
			const match = event.file?.includes(SCENARIO_FILE) ?? false;
			if (match) scenarioCallIds.add(event.call_id);
			return match;
		}
		if (event.event === "call_exit") {
			return scenarioCallIds.has(event.call_id);
		}
		return event.file?.includes(SCENARIO_FILE) ?? false;
	}

	function apply(delta: 1 | -1) {
		lastActionStart = events.length;
		lastInput = current;
		result = step(current, delta);
		current = result.next;
		// step() traces synchronously, so the action's events are already in.
		const actionEvents = events.slice(lastActionStart);
		history.push({
			action: history.length + 1,
			calls: actionEvents.filter((e) => e.event === "call_enter").length,
			derives: actionEvents.filter((e) => e.event === "value_derive").length,
			duration: actionEvents
				.filter((e) => e.event === "call_exit")
				.reduce((sum, e) => sum + (e.duration_ns ?? 0), 0),
		});
	}

	function reset() {
		clearTraceEvents();
		scenarioCallIds.clear();
		events = [];
		current = 0;
		result = null;
		lastActionStart = 0;
		lastInput = null;
		history = [];
	}

	function formatNs(ns: number): string {
		return ns >= 1000 ? (ns / 1000).toFixed(1) + " µs" : ns + " ns";
	}

	const lastActionEvents = $derived(events.slice(lastActionStart));
	const totals = $derived({
		roots: events.filter((e) => e.event === "root").length,
		calls: events.filter((e) => e.event === "call_enter").length,
		derives: events.filter((e) => e.event === "value_derive").length,
	});
	const lastAction = $derived({
		events: lastActionEvents.length,
		calls: lastActionEvents.filter((e) => e.event === "call_enter").length,
		derives: lastActionEvents.filter((e) => e.event === "value_derive").length,
		duration: lastActionEvents
			.filter((e) => e.event === "call_exit")
			.reduce((sum, e) => sum + (e.duration_ns ?? 0), 0),
	});
	// Latest known value per binding, displayed on the flow graph nodes.
	const flowValues = $derived<Record<string, string>>({
		...(lastInput !== null ? { count: String(lastInput) } : {}),
		...(result
			? {
					next: String(result.next),
					doubled: String(result.doubled),
					parity: result.parity,
					summary: result.summary,
				}
			: {}),
	});
</script>

<div class="grid gap-4 lg:grid-cols-3">
	<div class="flex flex-col rounded-xl border bg-card p-4">
		<h3 class="pb-3 text-sm font-medium">Playground</h3>
		<div class="flex flex-1 flex-col items-center justify-center gap-4 py-4">
			<span class="font-mono text-6xl font-bold tabular-nums">{current}</span>
			{#if result}
				<div class="flex flex-col items-center gap-1 text-sm text-muted-foreground">
					<span>doubled: <b class="text-foreground">{result.doubled}</b></span>
					<span>{result.summary}</span>
				</div>
			{/if}
			<div class="flex items-center gap-2">
				<Button variant="outline" size="icon" onclick={() => apply(-1)}>
					<MinusIcon />
				</Button>
				<Button variant="outline" size="icon" onclick={() => apply(1)}>
					<PlusIcon />
				</Button>
				<Button variant="ghost" size="icon" onclick={reset}>
					<RotateCcwIcon />
				</Button>
			</div>
		</div>
	</div>

	<div class="rounded-xl border bg-card p-4 lg:col-span-2">
		<TracePanel
			{events}
			actionStart={lastActionStart}
			values={flowValues}
			emptyHint="Click + / − to generate trace events."
		/>
	</div>
</div>

<div class="mt-4 flex flex-col rounded-xl border bg-card p-4">
	<h3 class="pb-3 text-sm font-medium">Metrics</h3>
	<div class="grid grid-cols-2 gap-3 sm:grid-cols-5">
		<div class="rounded-lg bg-muted/50 p-3">
			<div class="text-2xl font-bold tabular-nums">{totals.calls}</div>
			<div class="text-xs text-muted-foreground">total calls</div>
		</div>
		<div class="rounded-lg bg-muted/50 p-3">
			<div class="text-2xl font-bold tabular-nums">{totals.derives}</div>
			<div class="text-xs text-muted-foreground">total derives</div>
		</div>
		<div class="rounded-lg bg-muted/50 p-3">
			<div class="text-2xl font-bold tabular-nums">{lastAction.calls}</div>
			<div class="text-xs text-muted-foreground">calls / click</div>
		</div>
		<div class="rounded-lg bg-muted/50 p-3">
			<div class="text-2xl font-bold tabular-nums">{lastAction.derives}</div>
			<div class="text-xs text-muted-foreground">derives / click</div>
		</div>
		<div class="col-span-2 rounded-lg bg-muted/50 p-3 sm:col-span-1">
			<div class="text-2xl font-bold tabular-nums">
				{formatNs(lastAction.duration)}
			</div>
			<div class="text-xs text-muted-foreground">traced call time / click</div>
		</div>
	</div>
	{#if history.length > 0}
		<div class="grid gap-4 pt-4 md:grid-cols-2">
			<div>
				<p class="pb-1 text-xs text-muted-foreground">
					calls & derives / click
				</p>
				<LineChart
					data={history}
					x="action"
					series={[
						{
							key: "calls",
							label: "calls",
							value: (d: ActionStat) => d.calls,
							color: "var(--chart-2)",
						},
						{
							key: "derives",
							label: "derives",
							value: (d: ActionStat) => d.derives,
							color: "var(--chart-4)",
						},
					]}
					points
					legend
					height={180}
					props={{
						xAxis: { format: (v: number) => "#" + v },
					}}
				/>
			</div>
			<div>
				<p class="pb-1 text-xs text-muted-foreground">
					traced call time / click
				</p>
				<BarChart
					data={history}
					x="action"
					y="duration"
					height={180}
					props={{
						xAxis: { format: (v: number) => "#" + v },
						yAxis: { format: (v: number) => formatNs(v) },
						tooltip: { item: { format: (v: number) => formatNs(v) } },
					}}
				/>
			</div>
		</div>
	{/if}
</div>

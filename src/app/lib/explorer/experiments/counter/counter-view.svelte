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
	import { BarChart } from "layerchart";
	import SourceCode from "../../source-code.svelte";
	import TracePanel from "../../trace/trace-panel.svelte";
	import { step, type CounterStep } from "./scenario.js";
	import scenarioSource from "./scenario.ts?raw";

	const SCENARIO_FILE = "counter/scenario";
	const scenarioLines = scenarioSource.trimEnd().split(/\r?\n/);
	const stepStart = scenarioLines.findIndex((line) => line.startsWith("export function step("));
	const sourceFocus = stepStart === -1 ? undefined : {
		label: "step()",
		startLine: stepStart + 1,
		endLine: scenarioLines.length,
	};

	let current = $state(0);
	let result = $state<CounterStep | null>(null);
	let events = $state<TraceEvent[]>([]);
	// Index into the full event buffer where the last action started.
	let lastActionStart = $state(0);
	// Counter value before the last step, shown on the flow graph's root.
	let lastInput = $state<number | null>(null);
	let actionCount = $state(0);

	$effect(() => {
		events = takeTraceEvents().filter(isScenarioEvent);
		return subscribeTrace((event) => {
			if (isScenarioEvent(event)) events.push(event);
		});
	});

	// call_exit carries no file/line/column (trace schema v4), so it is
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
		actionCount++;
	}

	function reset() {
		clearTraceEvents();
		scenarioCallIds.clear();
		events = [];
		current = 0;
		result = null;
		lastActionStart = 0;
		lastInput = null;
		actionCount = 0;
	}

	const callCounts = $derived.by(() => {
		const counts = new Map<string, number>();
		for (const event of events) {
			// Each invocation has an enter and an exit; count it only on entry.
			if (event.event !== "call_enter") continue;
			const label = event.label ?? "unknown";
			counts.set(label, (counts.get(label) ?? 0) + 1);
		}
		return Array.from(counts, ([label, count]) => ({ label: label + "()", count }));
	});
	const totalCalls = $derived(callCounts.reduce((sum, call) => sum + call.count, 0));
	const callCountTicks = $derived.by(() => {
		const max = Math.max(0, ...callCounts.map((call) => call.count));
		const step = Math.max(1, Math.ceil(max / 5));
		return Array.from({ length: Math.floor(max / step) + 1 }, (_, i) => i * step);
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

<div class="min-w-0 overflow-hidden rounded-xl border bg-card">
	<div class="flex flex-wrap items-center gap-x-6 gap-y-3 px-4 py-3">
		<div class="flex items-center gap-4">
			<div>
				<h3 class="text-xs font-medium text-muted-foreground">Playground</h3>
				<span class="text-[10px] text-muted-foreground">count</span>
			</div>
			<span class="min-w-10 font-mono text-3xl font-semibold tabular-nums" aria-live="polite">{current}</span>
			<div class="flex items-center gap-1.5">
				<Button variant="outline" size="icon-sm" aria-label="Decrease count" onclick={() => apply(-1)}><MinusIcon /></Button>
				<Button variant="outline" size="icon-sm" aria-label="Increase count" onclick={() => apply(1)}><PlusIcon /></Button>
				<Button variant="ghost" size="icon-sm" aria-label="Reset counter and trace" onclick={reset}><RotateCcwIcon /></Button>
			</div>
		</div>
		<div class="flex flex-wrap items-center gap-3 text-xs sm:border-l sm:pl-6">
			{#if result}
				<span class="rounded-md bg-muted/70 px-2.5 py-1.5 text-muted-foreground">doubled <b class="ml-1 font-mono font-medium text-foreground">{result.doubled}</b></span>
				<span class="rounded-md bg-muted/70 px-2.5 py-1.5 text-muted-foreground">parity <b class="ml-1 font-mono font-medium text-foreground">{result.parity}</b></span>
				<span class="text-muted-foreground">{result.summary}</span>
			{:else}
				<span class="text-muted-foreground">Use + / − to trace an update.</span>
			{/if}
		</div>
		{#if actionCount > 0}
			<span class="ml-auto flex items-center gap-1.5 text-[11px] text-muted-foreground"><span class="size-1.5 rounded-full bg-chart-2"></span>Latest action #{actionCount}</span>
		{/if}
	</div>
	<SourceCode source={scenarioSource} filename="counter/scenario.ts" focus={sourceFocus} />
</div>

<div class="grid min-w-0 grid-cols-1 items-start gap-3">
	<div class="min-w-0 rounded-xl border bg-card p-3">
		<TracePanel
			{events}
			actionStart={lastActionStart}
			values={flowValues}
			emptyHint="Click + / − to generate trace events."
			class="h-[420px]"
		/>
	</div>

	<div class="flex min-w-0 flex-col gap-3 rounded-xl border bg-card p-3">
		<div class="flex items-center justify-between">
			<h3 class="text-sm font-medium">Metrics</h3>
			<span class="text-[10px] text-muted-foreground">{totalCalls} calls · {actionCount} {actionCount === 1 ? "action" : "actions"}</span>
		</div>
		<figure class="min-w-0">
			<figcaption class="flex flex-wrap items-center justify-between gap-2 text-xs text-muted-foreground">
				<span>Calls by function</span>
				<span class="text-[10px]">Cumulative since reset</span>
			</figcaption>
			{#if callCounts.length > 0}
				<div class="overflow-x-auto">
					<div class="min-w-[480px]">
						<BarChart
							data={callCounts}
							x="label"
							y="count"
							series={[{ key: "count", label: "Calls", color: "var(--chart-2)" }]}
							yDomain={[0, null]}
							yNice={false}
							height={232}
							labels={{ value: "count", placement: "outside" }}
							tooltip={false}
							tooltipContext={false}
							padding={{ top: 28, right: 16, bottom: 32, left: 40 }}
							props={{
								xAxis: { ticks: callCounts.map((call) => call.label) },
								yAxis: { ticks: callCountTicks, format: (v: number) => v.toLocaleString() },
								bars: { strokeWidth: 0 },
							}}
						/>
					</div>
				</div>
			{:else}
				<p class="py-8 text-center text-xs text-muted-foreground">Click + / − to compare function call counts.</p>
			{/if}
		</figure>
	</div>
</div>

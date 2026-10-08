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
	import SourceCode from "../../source-code.svelte";
	import TracePanel from "../../trace/trace-panel.svelte";
	import { step } from "./scenario.js";
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
	let events = $state<TraceEvent[]>([]);
	// Index into the full event buffer where the last action started.
	let lastActionStart = $state(0);
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
		current = step(current, delta).next;
		actionCount++;
	}

	function reset() {
		clearTraceEvents();
		scenarioCallIds.clear();
		events = [];
		current = 0;
		lastActionStart = 0;
		actionCount = 0;
	}
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
	</div>
	<SourceCode source={scenarioSource} filename="counter/scenario.ts" focus={sourceFocus} />
</div>

<div class="min-w-0 rounded-xl border bg-card p-3">
	<TracePanel
		events={events.slice(lastActionStart)}
		title={actionCount > 0 ? `Data paths · action #${actionCount}` : "Data paths"}
		emptyHint="Click + / − to trace an update."
	/>
</div>

<script lang="ts">
	import GanttChartIcon from "@lucide/svelte/icons/gantt-chart";
	import ListIcon from "@lucide/svelte/icons/list";
	import WorkflowIcon from "@lucide/svelte/icons/workflow";
	import type { TraceEvent } from "virtual:runes-runtime";
	import TraceEvents from "./trace-events.svelte";
	import TraceFlow from "./trace-flow.svelte";
	import TraceGantt from "./trace-gantt.svelte";

	interface Props {
		events: TraceEvent[];
		/** Index into events where the latest action begins. */
		actionStart?: number;
		/** Latest value per binding, shown on the flow graph nodes. */
		values?: Record<string, string>;
		emptyHint?: string;
		class?: string;
	}

	let {
		events,
		actionStart = 0,
		values = {},
		emptyHint = "Run the scenario to generate trace events.",
		class: className = "min-h-96",
	}: Props = $props();

	let view = $state<"flow" | "timeline" | "events">("flow");

	const views = [
		{ id: "flow", label: "Flow", icon: WorkflowIcon },
		{ id: "timeline", label: "Calls", icon: GanttChartIcon },
		{ id: "events", label: "Events", icon: ListIcon },
	] as const;

	// The explanatory views follow one action. Raw events retain the full buffer.
	const actionEvents = $derived(events.slice(actionStart));
	const viewDescription = $derived(
		view === "flow"
			? "Latest action · direct dependencies and produced values"
			: view === "timeline"
				? "Latest action · function order and measured duration"
				: "Raw instrumentation events across all actions",
	);
</script>

<div class="flex min-w-0 flex-col {className}">
	<div class="flex flex-wrap items-center justify-between gap-2 pb-2.5">
		<div>
			<h3 class="text-sm font-medium">Data-Flow Trace</h3>
			<p class="pt-0.5 text-[10px] text-muted-foreground">{viewDescription}</p>
		</div>
		<div class="flex gap-0.5 rounded-lg bg-muted/60 p-0.5">
			{#each views as v (v.id)}
				<button
					type="button"
					class={{
						"flex items-center gap-1.5 rounded-md px-2 py-1 text-xs transition-colors": true,
						"bg-background font-medium shadow-sm": view === v.id,
						"text-muted-foreground hover:text-foreground": view !== v.id,
					}}
					onclick={() => (view = v.id)}
					aria-pressed={view === v.id}
				>
					<v.icon class="size-3.5" />
					{v.label}
				</button>
			{/each}
		</div>
	</div>
	<div class="flex min-h-0 flex-1 flex-col overflow-hidden rounded-md border bg-muted/30" class:relative={view === "flow"}>
		{#if view === "flow"}
			<div class="absolute inset-0"><TraceFlow events={actionEvents} {values} {emptyHint} /></div>
		{:else if view === "timeline"}
			<TraceGantt events={actionEvents} {emptyHint} />
		{:else}
			<div class="min-h-0 flex-1 overflow-auto"><TraceEvents {events} {actionStart} {emptyHint} /></div>
		{/if}
	</div>
</div>

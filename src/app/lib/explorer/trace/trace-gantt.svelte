<script lang="ts">
	import { Gantt, Tooltip, Willow, type IApi, type IColumnConfig, type IScaleConfig } from "@svar-ui/svelte-gantt";
	import MinusIcon from "@lucide/svelte/icons/minus";
	import PlusIcon from "@lucide/svelte/icons/plus";
	import type { TraceEvent } from "virtual:runes-runtime";
	import TraceGanttTooltip from "./trace-gantt-tooltip.svelte";
	import { buildTraceGantt, formatNs, formatStep, stepDate } from "./trace-timeline.js";

	interface Props {
		events: TraceEvent[];
		emptyHint?: string;
	}

	let { events, emptyHint = "No trace events yet." }: Props = $props();
	let api = $state<IApi>();
	let expanded = $state<Record<string, boolean>>({});
	let width = $state(800);
	const trace = $derived(buildTraceGantt(events, expanded));
	const cellWidth = $derived(Math.max(32, Math.min(120, (width - 344) / (trace.steps + 1))));
	const scales: IScaleConfig[] = [{ unit: "day", step: 1, format: (date) => `Step ${formatStep(date)}` }];
	const zoom = {
		level: 0,
		levels: [{ minCellWidth: 20, maxCellWidth: 300, scales }],
	};
	const columns: IColumnConfig[] = [
		{ id: "text", header: "Function", width: 174 },
		{ id: "count", header: "Count", width: 76, align: "right" },
		{ id: "timing", header: "Call time", width: 90, align: "right" },
	];
</script>

{#if trace.tasks.length === 0}
	<div class="flex flex-1 items-center justify-center p-4 text-center text-xs text-muted-foreground">
		{emptyHint}
	</div>
{:else}
	<div class="flex min-h-0 min-w-0 flex-1 flex-col bg-card">
		<div class="flex flex-wrap items-center gap-2 border-b px-3 py-2 text-xs">
			<span class="font-medium">{trace.calls} calls · {trace.functions} functions</span>
			<span class="font-mono text-muted-foreground">{formatNs(trace.duration)} action span</span>
			<div class="ml-auto flex items-center gap-1">
				<span class="mr-1 text-[10px] text-muted-foreground">Call sequence</span>
				<button
					type="button"
					class="flex size-6 items-center justify-center rounded hover:bg-muted"
					aria-label="Zoom out"
					onclick={() => api?.exec("zoom-scale", { dir: -1 })}
				><MinusIcon class="size-3.5" /></button>
				<button
					type="button"
					class="flex size-6 items-center justify-center rounded hover:bg-muted"
					aria-label="Zoom in"
					onclick={() => api?.exec("zoom-scale", { dir: 1 })}
				><PlusIcon class="size-3.5" /></button>
			</div>
		</div>
		<div class="svar-trace min-h-0 min-w-0 flex-1" bind:clientWidth={width}>
			<Willow>
				<Tooltip {api} content={TraceGanttTooltip}>
					<Gantt
						tasks={trace.tasks}
						links={trace.links}
						{columns}
						{scales}
						{zoom}
						{cellWidth}
						gridWidth={340}
						cellHeight={32}
						scaleHeight={32}
						start={stepDate(0)}
						end={stepDate(trace.steps + 1)}
						autoScale={false}
						readonly
						init={(instance) => (api = instance)}
						onopentask={({ id, mode }) => (expanded[String(id)] = mode)}
					/>
				</Tooltip>
			</Willow>
		</div>
		<p class="border-t px-3 py-2 text-[10px] text-muted-foreground">
			Each row is a function call from the latest action. The axis shows order; call time is listed separately.
		</p>
	</div>
{/if}

<style>
	.svar-trace :global(.wx-willow-theme) {
		--wx-font-family: var(--font-sans);
		--wx-font-size: 12px;
		--wx-font-size-sm: 11px;
	}
</style>

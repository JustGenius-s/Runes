<script lang="ts">
	import type { ComponentProps } from "svelte";
	import type { Tooltip } from "@svar-ui/svelte-gantt";
	import { formatNs, type TraceGanttTask } from "./trace-timeline.js";

	type Content = NonNullable<ComponentProps<typeof Tooltip>["content"]>;
	let { data }: ComponentProps<Content> = $props();
	const task = $derived("task" in data ? data.task as TraceGanttTask : null);
</script>

{#if task}
	<div class="max-w-72 space-y-1 p-1 text-xs">
		<div class="font-mono font-semibold">{task.text}</div>
		<div>{task.count} {task.count === 1 ? "occurrence" : "occurrences"}</div>
		{#if task.timing !== "—"}<div>Call time: {task.timing}</div>{/if}
		<div>{task.group ? "First seen" : "Start"}: {formatNs(task.traceStart)}</div>
		<div>{task.group ? "Last seen" : "End"}: {task.pendingCount ? "pending" : formatNs(task.traceEnd)}</div>
		{#if task.errorCount}<div>{task.errorCount} threw / rejected</div>{/if}
	</div>
{:else if "link" in data}
	<div class="p-1 text-xs">Recorded execution order</div>
{/if}

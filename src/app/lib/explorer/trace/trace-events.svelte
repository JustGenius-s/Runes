<script lang="ts">
	import type { TraceEvent } from "virtual:runes-runtime";

	interface Props {
		events: TraceEvent[];
		/** Index where the latest action begins; a divider is drawn there. */
		actionStart?: number;
		emptyHint?: string;
	}

	let {
		events,
		actionStart = 0,
		emptyHint = "No trace events yet.",
	}: Props = $props();

	function describe(event: TraceEvent): string {
		switch (event.event) {
			case "root":
				return "root " + event.binding;
			case "call_enter":
				return "call " + event.label;
			case "call_exit":
				return (
					"exit " +
					event.label +
					" (" +
					formatNs(event.duration_ns ?? 0) +
					")"
				);
			case "value_derive":
				return "derive " + event.label;
		}
	}

	function formatNs(ns: number): string {
		return ns >= 1000 ? (ns / 1000).toFixed(1) + " µs" : ns + " ns";
	}
</script>

<div class="flex h-full flex-col gap-0.5 overflow-y-auto p-2 font-mono text-xs">
	{#if events.length === 0}
		<span class="p-2 text-muted-foreground">{emptyHint}</span>
	{/if}
	{#each events as event, i (i)}
		<div class="flex items-center gap-2">
			{#if i === actionStart && i > 0}
				<span class="w-full border-t border-dashed"></span>
			{/if}
		</div>
		<div class="flex items-center gap-2 whitespace-nowrap">
			<span
				class={{
					"size-1.5 shrink-0 rounded-full": true,
					"bg-chart-1": event.event === "root",
					"bg-chart-2": event.event === "call_enter",
					"bg-chart-3": event.event === "call_exit",
					"bg-chart-4": event.event === "value_derive",
				}}
			></span>
			<span>{describe(event)}</span>
			{#if event.roots && event.roots.length > 0 && event.event !== "root"}
				<span class="text-muted-foreground">
					← [{event.roots.join(", ")}]
				</span>
			{/if}
		</div>
	{/each}
</div>

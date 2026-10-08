<script lang="ts">
	import ChartLineIcon from "@lucide/svelte/icons/chart-line";
	import PlayIcon from "@lucide/svelte/icons/play";
	import WaypointsIcon from "@lucide/svelte/icons/waypoints";
	import CounterView from "./experiments/counter/counter-view.svelte";
	import FormValidationView from "./experiments/form-validation/form-validation-view.svelte";
	import type { Selection } from "./nav.svelte.js";

	let { selection }: { selection: Selection } = $props();

	const panels = [
		{
			icon: PlayIcon,
			title: "Playground",
			description: "An interactive scene — trigger operations here.",
		},
		{
			icon: WaypointsIcon,
			title: "Data-Flow Trace",
			description: "Derivation chains and dependency call graph recorded by Runes.",
		},
		{
			icon: ChartLineIcon,
			title: "Metrics",
			description: "Quantified comparison: update counts, fan-out, chain length.",
		},
	];
</script>

<div class="flex w-full min-w-0 flex-col gap-4 py-1">
	<div class="flex flex-col gap-1">
		<span class="text-xs font-medium text-muted-foreground">
			{selection.category.title}
		</span>
		<h1 class="text-2xl font-bold tracking-tight">
			{selection.experiment.title}
		</h1>
		<p class="text-sm text-muted-foreground">{selection.experiment.summary}</p>
	</div>

	{#if selection.experiment.id === "counter"}
		<CounterView />
	{:else if selection.experiment.id === "form-validation"}
		<FormValidationView />
	{:else}
		<div class="grid gap-4 md:grid-cols-3">
			{#each panels as panel (panel.title)}
				<div
					class="flex min-h-40 flex-col items-center justify-center gap-2 rounded-xl border border-dashed p-4 text-center"
				>
					<panel.icon class="size-5 text-muted-foreground" />
					<span class="text-sm font-medium">{panel.title}</span>
					<span class="text-xs text-muted-foreground">{panel.description}</span>
				</div>
			{/each}
		</div>
	{/if}
</div>

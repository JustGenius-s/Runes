<script lang="ts">
	import ChartLineIcon from "@lucide/svelte/icons/chart-line";
	import PlayIcon from "@lucide/svelte/icons/play";
	import WaypointsIcon from "@lucide/svelte/icons/waypoints";
	import type { Selection } from "./nav.svelte.js";

	let { selection }: { selection: Selection } = $props();

	const panels = [
		{
			icon: PlayIcon,
			title: "演示区",
			description: "可交互的实验场景，在这里触发操作。",
		},
		{
			icon: WaypointsIcon,
			title: "数据流追踪",
			description: "Runes 记录的派生链与依赖调用图。",
		},
		{
			icon: ChartLineIcon,
			title: "定量指标",
			description: "更新次数、扇出宽度、链长等量化对比。",
		},
	];
</script>

<div class="mx-auto flex w-full max-w-5xl flex-col gap-6 py-2">
	<div class="flex flex-col gap-1">
		<span class="text-xs font-medium text-muted-foreground">
			{selection.category.title}
		</span>
		<h1 class="text-2xl font-bold tracking-tight">
			{selection.experiment.title}
		</h1>
		<p class="text-muted-foreground">{selection.experiment.summary}</p>
	</div>

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
</div>

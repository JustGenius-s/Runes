<script lang="ts">
	import type { TraceEvent } from "virtual:runes-runtime";
	import { buildDataPaths, formatNs, layoutPathTree, type DataPath } from "./trace-paths.js";

	interface Props {
		events: TraceEvent[];
		title?: string;
		/** Time at the right edge of the bars; pass a shared value to compare panels. */
		scaleNs?: number;
		emptyHint?: string;
		class?: string;
	}

	let {
		events,
		title = "Data paths",
		scaleNs = 0,
		emptyHint = "Run the scenario to trace its data.",
		class: className = "",
	}: Props = $props();

	const palette = [
		"oklch(0.62 0.19 255)",
		"oklch(0.7 0.17 50)",
		"oklch(0.65 0.13 185)",
		"oklch(0.58 0.2 295)",
		"oklch(0.66 0.2 355)",
		"oklch(0.68 0.16 145)",
		"oklch(0.78 0.15 85)",
		"oklch(0.55 0.08 230)",
	];

	const paths = $derived(buildDataPaths(events));
	const rows = $derived(layoutPathTree(paths));
	const maxNs = $derived(Math.max(scaleNs, ...paths.map((path) => path.totalNs)));
	let hoveredId = $state<string | null>(null);
	const highlighted = $derived(
		new Set(paths.find((path) => path.id === hoveredId)?.steps.map((step) => step.id) ?? []),
	);

	// Hashing keeps a function's color stable across panels and actions.
	function colorOf(label: string): string {
		let hash = 0;
		for (const char of label) hash = (hash * 31 + char.charCodeAt(0)) >>> 0;
		return palette[hash % palette.length];
	}

	function percent(ns: number): number {
		return maxNs === 0 ? 0 : (ns / maxNs) * 100;
	}

	function segments(path: DataPath) {
		let startNs = 0;
		return path.steps
			.filter((step) => step.calls.length > 0)
			.map((step) => {
				const segment = {
					step,
					startNs,
					label: step.calls.map((call) => call.label).join(" · "),
					own: step.id === path.id,
				};
				startNs += step.durationNs;
				return segment;
			});
	}

	function duration(ns: number): string {
		return ns === 0 ? "≈ 0" : formatNs(ns);
	}
</script>

<section class="flex min-w-0 flex-col {className}">
	<div class="flex flex-wrap items-baseline justify-between gap-2 pb-2">
		<h3 class="text-sm font-medium">{title}</h3>
		<span class="text-[10px] text-muted-foreground">Tree = path · bar = time along the path</span>
	</div>
	{#if paths.length === 0}
		<div class="flex min-h-24 items-center justify-center rounded-md border border-dashed px-4 text-center text-xs text-muted-foreground">
			{emptyHint}
		</div>
	{:else}
		<div class="rounded-md border">
			<div class="flex gap-3 border-b px-2 py-1 text-[10px] text-muted-foreground">
				<span class="min-w-0 flex-1">Value</span>
				<span class="flex w-2/5 shrink-0 justify-between pr-1 font-mono tabular-nums">
					<span>0</span>
					<span>{duration(maxNs)}</span>
				</span>
				<span class="w-16 shrink-0 text-right">Path time</span>
			</div>
			<ol onmouseleave={() => (hoveredId = null)}>
				{#each rows as row (row.path.id)}
					{@const path = row.path}
					{@const own = path.steps.at(-1)!}
					{@const ownLabel = own.calls.map((call) => call.label).join(" · ")}
					<li
						class={{
							"flex h-11 items-stretch gap-3 px-2 transition-colors": true,
							"bg-muted/70": highlighted.has(path.id),
						}}
						onmouseenter={() => (hoveredId = path.id)}
					>
						<div class="flex min-w-0 flex-1 items-stretch">
							<!-- Connectors meet at the name line, 14px from the row top. -->
							{#each row.guides as continues, level (level)}
								<span class="relative w-4 shrink-0">
									{#if continues}<span class="absolute inset-y-0 left-1/2 border-l border-muted-foreground/40"></span>{/if}
								</span>
							{/each}
							{#if row.depth > 0}
								<span class="relative w-4 shrink-0">
									<span class={{ "absolute left-1/2 top-0 border-l border-muted-foreground/40": true, "h-3.5": row.last, "h-full": !row.last }}></span>
									<span class="absolute left-1/2 top-3.5 w-full border-t border-muted-foreground/40"></span>
								</span>
							{/if}
							<span class="relative w-4 shrink-0">
								<span
									class="absolute left-1/2 top-3.5 z-10 size-2.5 -translate-x-1/2 -translate-y-1/2 rounded-full ring-2 ring-card"
									style:background={ownLabel ? colorOf(ownLabel) : "var(--muted-foreground)"}
								></span>
								{#if row.hasChildren}<span class="absolute bottom-0 left-1/2 top-3.5 border-l border-muted-foreground/40"></span>{/if}
							</span>
							<div class="flex min-w-0 flex-1 flex-col gap-0.5 pt-1.5 pl-1">
								<div class="flex h-4 min-w-0 items-center gap-1.5 font-mono text-xs">
									<span class="shrink-0 font-semibold">{path.name}</span>
									{#if path.value !== undefined}
										<span class="truncate text-muted-foreground" title={path.value}>= {path.value}</span>
									{/if}
								</div>
								<div class="flex min-w-0 items-center gap-1.5 font-mono text-[10px] text-muted-foreground">
									{#if path.input}
										<span>input</span>
									{:else if ownLabel}
										<span class="truncate">{ownLabel}</span>
										<span class="shrink-0 tabular-nums text-foreground/80">+{duration(own.durationNs)}</span>
									{:else}
										<span>derived</span>
									{/if}
									{#if path.otherInputs.length > 0}
										<span class="truncate">· also reads {path.otherInputs.join(", ")}</span>
									{/if}
								</div>
							</div>
						</div>
						<div class="relative w-2/5 shrink-0">
							<span class="absolute inset-y-0 left-1/2 border-l border-dashed border-border/70"></span>
							<div class="absolute inset-y-0 left-0 right-1">
								{#each segments(path) as segment (segment.step.id)}
									<span
										class="absolute top-1/2 h-3 -translate-y-1/2 rounded-[2px] ring-1 ring-card transition-opacity"
										style:left="{percent(segment.startNs)}%"
										style:width="max(3px, {percent(segment.step.durationNs)}%)"
										style:background={colorOf(segment.label)}
										style:opacity={segment.own || highlighted.has(path.id) ? 1 : 0.3}
										title="{segment.label} · {duration(segment.step.durationNs)}"
									></span>
								{/each}
							</div>
						</div>
						<div class="flex w-16 shrink-0 flex-col items-end justify-center font-mono text-xs tabular-nums">
							{#if path.input}
								<span class="text-muted-foreground">—</span>
							{:else if path.pending}
								<span class="text-muted-foreground">pending</span>
							{:else}
								<span title={path.totalNs === 0 ? "Below the timer resolution" : undefined}>{duration(path.totalNs)}</span>
							{/if}
							{#if path.threw}<span class="text-[10px] text-destructive">threw</span>{/if}
						</div>
					</li>
				{/each}
			</ol>
		</div>
	{/if}
</section>

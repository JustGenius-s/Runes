<script lang="ts">
	import ArrowRightIcon from "@lucide/svelte/icons/arrow-right";
	import type { TraceEvent } from "virtual:runes-runtime";
	import { buildDataPaths, formatNs } from "./trace-paths.js";

	interface Props {
		events: TraceEvent[];
		title?: string;
		emptyHint?: string;
		class?: string;
	}

	let {
		events,
		title = "Data paths",
		emptyHint = "Run the scenario to trace its data.",
		class: className = "",
	}: Props = $props();

	const paths = $derived(buildDataPaths(events));

	function duration(ns: number): string {
		return ns === 0 ? "≈ 0" : formatNs(ns);
	}
</script>

{#snippet time(ns: number, pending: boolean)}
	{#if pending}
		<span>pending</span>
	{:else}
		<span title={ns === 0 ? "Below the timer resolution" : undefined}>{duration(ns)}</span>
	{/if}
{/snippet}

<section class="flex min-w-0 flex-col {className}">
	<div class="flex items-baseline justify-between gap-2 pb-2">
		<h3 class="text-sm font-medium">{title}</h3>
		{#if paths.length > 0}
			<span class="text-[10px] text-muted-foreground">{paths.length} values · path time</span>
		{/if}
	</div>
	{#if paths.length === 0}
		<div class="flex min-h-24 items-center justify-center rounded-md border border-dashed px-4 text-center text-xs text-muted-foreground">
			{emptyHint}
		</div>
	{:else}
		<ol class="divide-y rounded-md border">
			{#each paths as path (path.id)}
				<li class="flex items-start gap-4 px-3 py-2">
					<div class="min-w-0 flex-1 space-y-1">
						<div class="flex min-w-0 items-baseline gap-2 font-mono text-xs">
							<span class="shrink-0 font-semibold">{path.name}</span>
							{#if path.value !== undefined}
								<span class="truncate text-muted-foreground" title={path.value}>= {path.value}</span>
							{/if}
						</div>
						{#if !path.input}
							<div class="flex flex-wrap items-center gap-x-1.5 gap-y-1 font-mono text-[11px] text-muted-foreground">
								{#each path.steps as step, index (step.id)}
									{#if index > 0}
										<ArrowRightIcon class="size-3 shrink-0" />
										{#if step.calls.length > 0}
											<span>{step.calls.map((call) => call.label).join(" · ")}</span>
											<span class="rounded bg-muted px-1 tabular-nums text-foreground/80">
												{@render time(step.durationNs, step.calls.some((call) => call.durationNs === undefined))}
											</span>
											<ArrowRightIcon class="size-3 shrink-0" />
										{/if}
									{/if}
									<span class="rounded border bg-background px-1 text-foreground">{step.name}</span>
								{/each}
							</div>
						{/if}
						{#if path.otherInputs.length > 0}
							<div class="text-[10px] text-muted-foreground">also reads {path.otherInputs.join(", ")}</div>
						{/if}
					</div>
					<div class="shrink-0 pt-px text-right font-mono text-xs tabular-nums">
						{#if path.input}
							<span class="text-muted-foreground">input</span>
						{:else}
							{@render time(path.totalNs, path.pending)}
							{#if path.threw}<div class="text-[10px] text-destructive">threw</div>{/if}
						{/if}
					</div>
				</li>
			{/each}
		</ol>
	{/if}
</section>

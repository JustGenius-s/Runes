<script lang="ts">
	import ChevronRightIcon from "@lucide/svelte/icons/chevron-right";
	import FlameIcon from "@lucide/svelte/icons/flame";
	import XIcon from "@lucide/svelte/icons/x";
	import { SvelteSet } from "svelte/reactivity";
	import type { TraceEvent } from "virtual:runes-runtime";
	import {
		buildDataPaths,
		connectorHighlights,
		formatNs,
		layoutPathTree,
		type DataPath,
		type PathStep,
	} from "./trace-paths.js";

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

	// Selection and collapsing are keyed by value name so they survive new
	// actions, whose events carry fresh ids.
	let selectedName = $state<string | null>(null);
	let hoveredId = $state<string | null>(null);
	let hoveredSegment = $state<{ rowId: string; stepId: string } | null>(null);
	const collapsed = new SvelteSet<string>();
	let list = $state<HTMLOListElement>();

	const paths = $derived(buildDataPaths(events));
	const rows = $derived(layoutPathTree(paths, (path) => collapsed.has(path.name)));
	const maxNs = $derived(Math.max(scaleNs, ...paths.map((path) => path.totalNs)));
	const slowestOwnNs = $derived(Math.max(0, ...paths.map((path) => ownStep(path).durationNs)));
	const selectedPath = $derived(paths.find((path) => path.name === selectedName));
	const activePath = $derived(paths.find((path) => path.id === hoveredId) ?? selectedPath);
	const activeSteps = $derived(new Set(activePath?.steps.map((step) => step.id) ?? []));
	const connectors = $derived(connectorHighlights(rows, activePath));

	// Hashing keeps a function's color stable across panels and actions.
	function colorOf(label: string): string {
		let hash = 0;
		for (const char of label) hash = (hash * 31 + char.charCodeAt(0)) >>> 0;
		return palette[hash % palette.length];
	}

	function callLabel(step: PathStep): string {
		return step.calls.map((call) => call.label).join(" · ");
	}

	function ownStep(path: DataPath): PathStep {
		return path.steps.at(-1)!;
	}

	function percent(ns: number): number {
		return maxNs === 0 ? 0 : (ns / maxNs) * 100;
	}

	function share(ns: number, totalNs: number): string {
		return totalNs === 0 ? "—" : `${Math.round((ns / totalNs) * 100)}%`;
	}

	function duration(ns: number): string {
		return ns === 0 ? "≈ 0" : formatNs(ns);
	}

	function segments(path: DataPath) {
		let startNs = 0;
		return path.steps
			.filter((step) => step.calls.length > 0)
			.map((step) => {
				const segment = { step, startNs, label: callLabel(step), own: step.id === path.id };
				startNs += step.durationNs;
				return segment;
			});
	}

	/** Keeps the own-time label readable whether the segment is short, late, or spans the bar. */
	function ownLabelPosition(startNs: number, ns: number): { left?: string; right?: string } {
		const start = percent(startNs);
		const end = percent(startNs + ns);
		if (end <= 70) return { left: `calc(${end}% + 6px)` };
		if (start >= 30) return { right: `calc(${100 - start}% + 6px)` };
		return { right: `calc(${100 - end}% + 4px)` };
	}

	function toggleSelected(path: DataPath) {
		selectedName = selectedName === path.name ? null : path.name;
	}

	function toggleCollapsed(path: DataPath) {
		if (collapsed.has(path.name)) collapsed.delete(path.name);
		else collapsed.add(path.name);
	}

	function focusRow(index: number) {
		const row = rows[index];
		if (!row) return;
		selectedName = row.path.name;
		list?.querySelectorAll<HTMLElement>("[role=treeitem]")[index]?.focus();
	}

	function onRowKeydown(event: KeyboardEvent, index: number) {
		const row = rows[index];
		const expanded = row.hasChildren && !collapsed.has(row.path.name);
		switch (event.key) {
			case "ArrowDown":
				focusRow(index + 1);
				break;
			case "ArrowUp":
				focusRow(index - 1);
				break;
			case "ArrowRight":
				if (row.hasChildren && !expanded) toggleCollapsed(row.path);
				else if (expanded) focusRow(index + 1);
				break;
			case "ArrowLeft":
				if (expanded) toggleCollapsed(row.path);
				else focusRow(rows.findIndex((candidate) => candidate.path.id === row.path.steps.at(-2)?.id));
				break;
			case "Enter":
			case " ":
				toggleSelected(row.path);
				break;
			case "Escape":
				selectedName = null;
				break;
			default:
				return;
		}
		event.preventDefault();
	}
</script>

{#snippet line(position: string, side: "left" | "top", active: boolean)}
	<span
		class={[
			"absolute",
			position,
			side === "left" ? (active ? "border-l-2" : "border-l") : active ? "border-t-2" : "border-t",
			active ? "z-[1] border-foreground/80" : "border-muted-foreground/50",
		]}
	></span>
{/snippet}

<section class="flex min-w-0 flex-col {className}">
	<div class="flex flex-wrap items-baseline justify-between gap-2 pb-2">
		<h3 class="text-sm font-medium">{title}</h3>
		<span class="text-[10px] text-muted-foreground">
			Tree = path · bar = time along it · click a value for its breakdown
		</span>
	</div>
	{#if paths.length === 0}
		<div class="flex min-h-24 items-center justify-center rounded-md border border-dashed px-4 text-center text-xs text-muted-foreground">
			{emptyHint}
		</div>
	{:else}
		<div class="rounded-md border">
			<div class="flex gap-3 border-b px-2 py-1 text-[10px] text-muted-foreground">
				<span class="min-w-0 flex-1">Value · producing function</span>
				<span class="relative w-2/5 shrink-0 font-mono tabular-nums">
					<span>0</span>
					<span class="absolute left-1/2 -translate-x-1/2">{duration(maxNs / 2)}</span>
					<span class="absolute right-1">{duration(maxNs)}</span>
				</span>
				<span class="w-16 shrink-0 text-right">Path time</span>
			</div>
			<ol
				bind:this={list}
				role="tree"
				aria-label={title}
				onmouseleave={() => {
					hoveredId = null;
					hoveredSegment = null;
				}}
			>
				{#each rows as row, index (row.path.id)}
					{@const path = row.path}
					{@const own = ownStep(path)}
					{@const ownLabel = callLabel(own)}
					{@const connector = connectors[index]}
					{@const isCollapsed = collapsed.has(path.name)}
					{@const isSelected = selectedPath?.id === path.id}
					{@const isSlowest = ownLabel !== "" && slowestOwnNs > 0 && own.durationNs === slowestOwnNs}
					{@const pointed = hoveredSegment?.stepId === path.id}
					<li
						role="treeitem"
						aria-selected={isSelected}
						aria-expanded={row.hasChildren ? !isCollapsed : undefined}
						aria-level={row.depth + 1}
						tabindex={isSelected || (!selectedPath && index === 0) ? 0 : -1}
						class={{
							"flex h-12 cursor-pointer items-stretch gap-3 px-2 outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-inset": true,
							"bg-muted/70": activeSteps.has(path.id) && !isSelected,
							"bg-accent ring-1 ring-primary/30 ring-inset": isSelected,
							"hover:bg-muted/40": !activeSteps.has(path.id),
						}}
						onmouseenter={() => (hoveredId = path.id)}
						onclick={() => toggleSelected(path)}
						onkeydown={(event) => onRowKeydown(event, index)}
					>
						<div class="flex min-w-0 flex-1 items-stretch">
							<!-- Connectors meet at the name line, 14px from the row top. -->
							{#each row.guides as continues, level (level)}
								<span class="relative w-4 shrink-0">
									{#if continues}{@render line("inset-y-0 left-1/2", "left", connector.guides[level])}{/if}
								</span>
							{/each}
							{#if row.depth > 0}
								<span class="relative w-4 shrink-0">
									{@render line("left-1/2 top-0 h-3.5", "left", connector.elbowTop)}
									{#if !row.last}{@render line("left-1/2 top-3.5 bottom-0", "left", connector.elbowBottom)}{/if}
									{@render line("left-1/2 top-3.5 w-full", "top", connector.elbowHorizontal)}
								</span>
							{/if}
							<span class="relative w-4 shrink-0">
								<span
									class={{
										"absolute left-1/2 top-3.5 z-10 -translate-x-1/2 -translate-y-1/2 rounded-full ring-2 transition-all": true,
										"size-3.5 ring-foreground": pointed,
										"size-3 ring-card": !pointed && activeSteps.has(path.id),
										"size-2.5 ring-card": !pointed && !activeSteps.has(path.id),
									}}
									style:background={ownLabel ? colorOf(ownLabel) : "var(--muted-foreground)"}
								></span>
								{#if row.hasChildren && !isCollapsed}
									{@render line("bottom-0 left-1/2 top-3.5", "left", connector.nodeDown)}
								{/if}
							</span>
							<div class="flex min-w-0 flex-1 flex-col gap-1 pt-1.5 pl-1">
								<div class="flex h-4 min-w-0 items-center gap-1.5 font-mono text-xs">
									{#if row.hasChildren}
										<button
											type="button"
											tabindex="-1"
											class="-my-0.5 flex size-5 shrink-0 items-center justify-center rounded border border-transparent text-muted-foreground hover:border-border hover:bg-background hover:text-foreground"
											aria-label={isCollapsed ? `Expand ${path.name}` : `Collapse ${path.name}`}
											title={isCollapsed ? "Expand" : "Collapse"}
											onclick={(event) => {
												event.stopPropagation();
												toggleCollapsed(path);
											}}
										>
											<ChevronRightIcon class={{ "size-3.5 transition-transform": true, "rotate-90": !isCollapsed }} />
										</button>
									{/if}
									<span class="shrink-0 font-semibold">{path.name}</span>
									{#if isCollapsed}
										<span class="shrink-0 rounded bg-muted px-1 text-[10px] text-muted-foreground">+{row.descendants}</span>
									{/if}
									{#if path.value !== undefined}
										<span class="truncate text-[11px] text-muted-foreground/80" title={path.value}>= {path.value}</span>
									{/if}
								</div>
								<div class="flex min-w-0 items-center gap-1.5 font-mono text-[10px]">
									{#if path.input}
										<span class="text-muted-foreground">input</span>
									{:else if ownLabel}
										<span
											class={{
												"inline-flex min-w-0 items-center gap-1 rounded px-1 transition-shadow": true,
												"ring-1 ring-foreground/60": pointed,
											}}
											style:background="color-mix(in oklch, {colorOf(ownLabel)} 16%, transparent)"
										>
											<span class="truncate">{ownLabel}</span>
											{#if isSlowest}
												<FlameIcon class="size-3 shrink-0 text-orange-500" aria-label="Slowest function in this trace" />
											{/if}
										</span>
									{:else}
										<span class="text-muted-foreground">derived</span>
									{/if}
									{#if path.otherInputs.length > 0}
										<span class="truncate text-muted-foreground">also reads {path.otherInputs.join(", ")}</span>
									{/if}
								</div>
							</div>
						</div>
						<div class="relative w-2/5 shrink-0">
							<span class="absolute inset-y-0 left-1/2 border-l border-dashed border-border/70"></span>
							<div class="absolute inset-y-0 left-0 right-1">
								{#each segments(path) as segment (segment.step.id)}
									{@const isHovered = hoveredSegment?.rowId === path.id && hoveredSegment.stepId === segment.step.id}
									<!-- svelte-ignore a11y_no_static_element_interactions -->
									<span
										class={{
											"absolute top-1/2 h-4 -translate-y-1/2 rounded-[3px] ring-card transition-[opacity,box-shadow]": true,
											"ring-1": !isHovered,
											"z-10 ring-2 ring-foreground": isHovered,
										}}
										style:left="{percent(segment.startNs)}%"
										style:width="max(3px, {percent(segment.step.durationNs)}%)"
										style:background={colorOf(segment.label)}
										style:opacity={segment.own || isHovered || activeSteps.has(path.id) ? 1 : 0.28}
										onmouseenter={() => (hoveredSegment = { rowId: path.id, stepId: segment.step.id })}
										onmouseleave={() => (hoveredSegment = null)}
									></span>
									{#if isHovered}
										<div
											class="pointer-events-none absolute bottom-full z-30 mb-1 -translate-x-1/2 whitespace-nowrap rounded-md border bg-popover px-2 py-1.5 text-[10px] text-popover-foreground shadow-md"
											style:left="{percent(segment.startNs + segment.step.durationNs / 2)}%"
										>
											<div class="flex items-center gap-1.5 font-mono font-semibold">
												<span class="size-2 rounded-sm" style:background={colorOf(segment.label)}></span>
												{segment.label}
											</div>
											<div class="mt-0.5 tabular-nums">
												+{duration(segment.step.durationNs)} · {share(segment.step.durationNs, path.totalNs)} of {path.name}'s path
											</div>
											<div class="text-muted-foreground">produces {segment.step.name}</div>
										</div>
									{/if}
								{/each}
								{#if !path.input && ownLabel}
									{@const ownStart = path.totalNs - own.durationNs}
									{@const position = ownLabelPosition(ownStart, own.durationNs)}
									<span
										class={{
											"pointer-events-none absolute top-1/2 z-20 -translate-y-1/2 whitespace-nowrap rounded bg-card/90 px-1 font-mono text-[10px] font-semibold tabular-nums shadow-sm": true,
											"text-orange-600": isSlowest,
										}}
										style:left={position.left}
										style:right={position.right}
									>
										+{duration(own.durationNs)}
									</span>
								{/if}
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

		{#if selectedPath}
			{@render breakdown(selectedPath)}
		{/if}
	{/if}
</section>

{#snippet breakdown(path: DataPath)}
	<div class="mt-2 rounded-md border bg-muted/20 p-3" aria-live="polite">
		<div class="flex items-start justify-between gap-3">
			<div class="min-w-0">
				<div class="text-[10px] text-muted-foreground">Path breakdown</div>
				<div class="font-mono text-sm font-semibold">{path.name}</div>
			</div>
			<div class="flex items-center gap-3">
				<div class="text-right">
					<div class="text-[10px] text-muted-foreground">Path time</div>
					<div class="font-mono text-sm font-semibold tabular-nums">{path.input ? "—" : duration(path.totalNs)}</div>
				</div>
				<button
					type="button"
					class="flex size-6 items-center justify-center rounded text-muted-foreground hover:bg-muted hover:text-foreground"
					aria-label="Close breakdown"
					onclick={() => (selectedName = null)}
				><XIcon class="size-3.5" /></button>
			</div>
		</div>
		{#if path.value !== undefined}
			<pre class="mt-2 max-h-20 overflow-auto rounded bg-background px-2 py-1 font-mono text-[10px] whitespace-pre-wrap break-all text-muted-foreground">{path.value}</pre>
		{/if}
		<ol class="mt-3 space-y-1.5">
			{#each path.steps as step, index (step.id)}
				{@const label = callLabel(step)}
				<li class="grid grid-cols-[1.25rem_minmax(0,1fr)_minmax(4rem,30%)_4.5rem_2.5rem] items-center gap-2 font-mono text-[11px]">
					<span class="text-right text-[10px] text-muted-foreground">{index + 1}</span>
					<span class="flex min-w-0 items-center gap-1.5">
						<span class="size-2 shrink-0 rounded-full" style:background={label ? colorOf(label) : "var(--muted-foreground)"}></span>
						<span class="shrink-0 font-semibold">{step.name}</span>
						<span class="truncate text-muted-foreground">
							{index === 0 ? "input" : label ? `← ${label}` : "derived"}
						</span>
					</span>
					<span class="h-2 overflow-hidden rounded-full bg-muted">
						{#if label}
							<span
								class="block h-full rounded-full"
								style:width="{path.totalNs === 0 ? 0 : (step.durationNs / path.totalNs) * 100}%"
								style:background={colorOf(label)}
							></span>
						{/if}
					</span>
					<span class="text-right font-semibold tabular-nums">{label ? `+${duration(step.durationNs)}` : ""}</span>
					<span class="text-right text-[10px] text-muted-foreground tabular-nums">{label ? share(step.durationNs, path.totalNs) : ""}</span>
				</li>
			{/each}
		</ol>
	</div>
{/snippet}

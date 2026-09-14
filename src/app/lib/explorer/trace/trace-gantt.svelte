<script lang="ts">
	import MaximizeIcon from "@lucide/svelte/icons/maximize";
	import MinusIcon from "@lucide/svelte/icons/minus";
	import PlusIcon from "@lucide/svelte/icons/plus";
	import { BarChart, Points, Tooltip } from "layerchart";
	import type { TraceEvent } from "virtual:runes-runtime";

	interface Props {
		events: TraceEvent[];
		emptyHint?: string;
	}

	let { events, emptyHint = "No trace events yet." }: Props = $props();

	interface Span {
		callId: number;
		label: string;
		start: number;
		end: number;
		unwind: boolean;
	}

	// One row per call. The transparent "offset" segment pushes the visible
	// "duration" segment to the call's start time — the stacked-bar gantt idiom.
	interface Row {
		key: string;
		label: string;
		offset: number;
		duration: number;
		unwind: boolean;
	}

	interface Marker {
		t: number;
		name: string;
	}

	// Reserved band at the top of the chart for the root/derive markers.
	const EVENTS_BAND = "events";

	// Calls cycle through chart-2/3/5 via the chart's ordinal color scale;
	// chart-1/chart-4 stay reserved for the root / derive markers.
	const PALETTE = ["var(--chart-2)", "var(--chart-3)", "var(--chart-5)"];

	// Pairs call_enter/call_exit by call_id into spans.
	const spans = $derived.by(() => {
		const open = new Map<number, TraceEvent>();
		const result: Span[] = [];
		for (const event of events) {
			if (event.event === "call_enter" && event.call_id !== undefined) {
				open.set(event.call_id, event);
			} else if (event.event === "call_exit" && event.call_id !== undefined) {
				const enter = open.get(event.call_id);
				if (!enter) continue;
				result.push({
					callId: event.call_id,
					label: event.label ?? "?",
					start: enter.at_ns,
					end: event.at_ns,
					unwind: event.unwind ?? false,
				});
				open.delete(event.call_id);
			}
		}
		return result;
	});

	const range = $derived.by(() => {
		const times = [
			...spans.flatMap((s) => [s.start, s.end]),
			...events
				.filter((e) => e.event === "root" || e.event === "value_derive")
				.map((e) => e.at_ns),
		];
		if (times.length === 0) return { min: 0, max: 1 };
		const min = Math.min(...times);
		const max = Math.max(...times);
		return { min, max: max === min ? min + 1 : max };
	});

	const rows = $derived<Row[]>(
		spans.map((s) => ({
			key: "#" + s.callId,
			label: s.label,
			offset: s.start - range.min,
			duration: Math.max(s.end - s.start, 0),
			unwind: s.unwind,
		})),
	);

	const labelByKey = $derived(new Map(rows.map((r) => [r.key, r.label])));

	const yBands = $derived([EVENTS_BAND, ...rows.map((r) => r.key)]);

	const rootMarkers = $derived<Marker[]>(
		events
			.filter((e) => e.event === "root")
			.map((e) => ({ t: e.at_ns - range.min, name: e.binding ?? "?" })),
	);

	const deriveMarkers = $derived<Marker[]>(
		events
			.filter((e) => e.event === "value_derive")
			.map((e) => ({ t: e.at_ns - range.min, name: e.label ?? "?" })),
	);

	function formatNs(ns: number): string {
		return ns >= 1000 ? (ns / 1000).toFixed(1) + " µs" : ns + " ns";
	}

	// Zoom scales the timeline track beyond 100% width; the scroller then
	// pans horizontally. Ctrl/Cmd + wheel zooms like a map.
	let zoom = $state(1);
	let scroller: HTMLDivElement | null = $state(null);

	function zoomBy(factor: number) {
		zoom = Math.min(128, Math.max(1, zoom * factor));
	}

	$effect(() => {
		const el = scroller;
		if (!el) return;
		const onWheel = (e: WheelEvent) => {
			if (!e.ctrlKey && !e.metaKey) return;
			e.preventDefault();
			zoomBy(e.deltaY < 0 ? 1.2 : 1 / 1.2);
		};
		el.addEventListener("wheel", onWheel, { passive: false });
		return () => el.removeEventListener("wheel", onWheel);
	});

	const chartHeight = $derived(Math.max(140, yBands.length * 28 + 40));

	const toolButton =
		"flex size-5 items-center justify-center rounded " +
		"text-muted-foreground hover:bg-muted hover:text-foreground";
</script>

{#if rows.length === 0 && rootMarkers.length === 0 && deriveMarkers.length === 0}
	<div
		class="flex h-full items-center justify-center text-xs text-muted-foreground"
	>
		{emptyHint}
	</div>
{:else}
	<div class="flex h-full flex-col">
		<div class="flex items-center gap-1 px-1 pb-1">
			<span class="mr-auto font-mono text-[10px] text-muted-foreground">
				total {formatNs(range.max - range.min)} · {rows.length} calls
			</span>
			<button
				type="button"
				class={toolButton}
				title="Zoom out"
				onclick={() => zoomBy(1 / 1.4)}
			>
				<MinusIcon class="size-3" />
			</button>
			<span class="w-10 text-center font-mono text-[10px] tabular-nums">
				{Math.round(zoom * 100)}%
			</span>
			<button
				type="button"
				class={toolButton}
				title="Zoom in"
				onclick={() => zoomBy(1.4)}
			>
				<PlusIcon class="size-3" />
			</button>
			<button
				type="button"
				class={toolButton}
				title="Reset zoom (Ctrl/⌘ + scroll also zooms)"
				onclick={() => (zoom = 1)}
			>
				<MaximizeIcon class="size-3" />
			</button>
		</div>
		<div class="min-h-0 flex-1 overflow-auto" bind:this={scroller}>
			<div class="min-w-full p-1" style="width: {zoom * 100}%">
				<BarChart
					data={rows}
					orientation="horizontal"
					y="key"
					yDomain={yBands}
					series={[
						{
							key: "offset",
							value: (d: Row) => d.offset,
							props: { fill: "transparent", rounded: "none", radius: 0 },
						},
						{
							key: "duration",
							label: "duration",
							value: (d: Row) => d.duration,
							props: {
								stroke: "var(--destructive)",
								strokeWidth: (d: Row) => (d.unwind ? 1.5 : 0),
							},
						},
					]}
					seriesLayout="stack"
					c={(d: Row) => d.label}
					cRange={PALETTE}
					bandPadding={0.3}
					labels={{
						seriesKey: "duration",
						placement: "outside",
						format: (v: number) => formatNs(v),
					}}
					props={{
						xAxis: { placement: "top", format: (v: number) => formatNs(v) },
						yAxis: {
							format: (key: string) =>
								key === EVENTS_BAND
									? EVENTS_BAND
									: (labelByKey.get(key) ?? key) + "()",
						},
					}}
					height={chartHeight}
					{aboveMarks}
					{tooltip}
				/>
			</div>
		</div>
	</div>
{/if}

{#snippet aboveMarks({ context }: { context: unknown })}
	<Points
		data={rootMarkers}
		x={(d: Marker) => d.t}
		y={() => EVENTS_BAND}
		r={3}
		fill="var(--chart-1)"
		stroke="var(--background)"
		strokeWidth={1}
	/>
	<Points
		data={deriveMarkers}
		x={(d: Marker) => d.t}
		y={() => EVENTS_BAND}
		r={3}
		fill="var(--chart-4)"
		stroke="var(--background)"
		strokeWidth={1}
	/>
{/snippet}

{#snippet tooltip({ context }: { context: any })}
	<Tooltip.Root {context}>
		{#snippet children({ data }: { data: Row | undefined })}
			{#if data}
				<Tooltip.Header value={data.label + "()"} />
				<Tooltip.List>
					<Tooltip.Item label="start" value={formatNs(data.offset)} />
					<Tooltip.Item
						label="end"
						value={formatNs(data.offset + data.duration)}
					/>
					<Tooltip.Item label="duration" value={formatNs(data.duration)} />
					{#if data.unwind}
						<Tooltip.Item label="status" value="unwound" />
					{/if}
				</Tooltip.List>
			{/if}
		{/snippet}
	</Tooltip.Root>
{/snippet}

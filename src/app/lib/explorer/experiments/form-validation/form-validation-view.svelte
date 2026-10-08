<script lang="ts">
	import ArrowRightIcon from "@lucide/svelte/icons/arrow-right";
	import CheckIcon from "@lucide/svelte/icons/check";
	import EyeIcon from "@lucide/svelte/icons/eye";
	import RotateCcwIcon from "@lucide/svelte/icons/rotate-ccw";
	import XIcon from "@lucide/svelte/icons/x";
	import { Button } from "$lib/components/ui/button/index.js";
	import {
		clearTraceEvents,
		takeTraceEvents,
		type TraceEvent,
	} from "virtual:runes-runtime";
	import { BarChart } from "layerchart";
	import SourceCode from "../../source-code.svelte";
	import TracePanel from "../../trace/trace-panel.svelte";
	import FormPreview from "./form-preview.svelte";
	import {
		initialFormState,
		runVariant,
		variantMeta,
		type FormChange,
		type FormFieldPath,
		type FormScenarioResult,
		type FormState,
		type FormStructure,
		type FormVariant,
		type StateChannel,
	} from "./scenario.js";
	import scenarioSource from "./scenario.ts?raw";

	type CompareAxis = "structure" | "channel";

	interface RunMetrics {
		calls: number;
		writes: number;
		checkedFields: number;
		receivers: number;
	}

	interface VariantRun {
		result: FormScenarioResult;
		events: TraceEvent[];
		metrics: RunMetrics;
	}

	const runnerNames: Record<FormVariant, string> = {
		"nested-binding": "runNestedBinding",
		"flat-binding": "runFlatBinding",
		"nested-store": "runNestedStore",
		"flat-store": "runFlatStore",
	};
	const scenarioLines = scenarioSource.trimEnd().split(/\r?\n/);
	const sourceFocusByVariant = Object.fromEntries(
		(Object.entries(runnerNames) as [FormVariant, string][]).map(([variant, name]) => {
			const start = scenarioLines.findIndex((line) =>
				line.startsWith(`export function ${name}(`),
			);
			const next = scenarioLines.findIndex(
				(line, index) => index > start && line.startsWith("export function "),
			);
			return [
				variant,
				start === -1
					? undefined
					: {
							label: `${name}()`,
							startLine: start + 1,
							endLine: next === -1 ? scenarioLines.length : next,
						},
			];
		}),
	) as Record<
		FormVariant,
		{ label: string; startLine: number; endLine: number } | undefined
	>;

	const writeCalls = new Set([
		"updateChildDraft",
		"mergeSectionIntoParent",
		"setPath",
		"dispatchSectionPatch",
		"dispatchFieldUpdate",
	]);

	let compareAxis = $state<CompareAxis>("structure");
	let fixedChannel = $state<StateChannel>("binding");
	let fixedStructure = $state<FormStructure>("flat");
	let currentState = $state<FormState>(initialFormState());
	let activePath = $state<FormFieldPath>("shipping.address.postcode");
	let runs = $state<VariantRun[]>([]);
	let selectedIndex = $state(0);
	let runCount = $state(0);

	const pair = $derived.by((): [FormVariant, FormVariant] => {
		if (compareAxis === "structure") {
			return [`nested-${fixedChannel}`, `flat-${fixedChannel}`];
		}
		return [`${fixedStructure}-binding`, `${fixedStructure}-store`];
	});
	const selectedRun = $derived(runs[selectedIndex] ?? null);
	const selectedVariant = $derived(selectedRun?.result.variant ?? pair[selectedIndex]);
	const sourceFocus = $derived(sourceFocusByVariant[selectedVariant]);
	const submitEnabled = $derived(
		selectedRun?.result.submitEnabled ?? Object.keys(currentState.errors).length === 0,
	);
	const outputsMatch = $derived(
		runs.length === 2 &&
			JSON.stringify({
				form: runs[0].result.form,
				errors: runs[0].result.errors,
				submit: runs[0].result.submitEnabled,
			}) ===
				JSON.stringify({
					form: runs[1].result.form,
					errors: runs[1].result.errors,
					submit: runs[1].result.submitEnabled,
				}),
	);
	const metricRows = $derived(
		runs.length === 2
			? [
					{ label: "Calls", left: runs[0].metrics.calls, right: runs[1].metrics.calls },
					{ label: "Writes", left: runs[0].metrics.writes, right: runs[1].metrics.writes },
					{
						label: "Fields checked",
						left: runs[0].metrics.checkedFields,
						right: runs[1].metrics.checkedFields,
					},
					{
						label: "Receivers",
						left: runs[0].metrics.receivers,
						right: runs[1].metrics.receivers,
					},
				]
			: [],
	);
	const metricTicks = $derived.by(() => {
		const max = Math.max(
			0,
			...metricRows.flatMap((row) => [row.left, row.right]),
		);
		return Array.from({ length: max + 1 }, (_, index) => index);
	});
	const finding = $derived.by(() => {
		if (runs.length !== 2) return "";
		if (compareAxis === "structure" && fixedChannel === "binding") {
			return "The nested form validates the active section before one parent merge. The flat form follows the changed path and its direct dependencies.";
		}
		if (compareAxis === "structure") {
			return "The nested store spends more calls on section routing and keeps subscriber fan-out scoped. The flat store uses fewer routing calls and reaches every form subscriber.";
		}
		if (fixedStructure === "nested") {
			return "Both variants preserve section-level validation. Store delivery adds patch, selector, and subscriber boundaries to the same update.";
		}
		return "Both flat variants validate the same dependency set. The store path exposes the update to a wider subscriber surface.";
	});

	function clearResults() {
		clearTraceEvents();
		runs = [];
		selectedIndex = 0;
	}

	function selectAxis(axis: CompareAxis) {
		compareAxis = axis;
		clearResults();
	}

	function selectChannel(channel: StateChannel) {
		fixedChannel = channel;
		clearResults();
	}

	function selectStructure(structure: FormStructure) {
		fixedStructure = structure;
		clearResults();
	}

	function analyze(result: FormScenarioResult, events: TraceEvent[]): RunMetrics {
		const calls = events.filter((event) => event.event === "call_enter");
		return {
			calls: calls.length,
			writes: calls.filter((event) => writeCalls.has(event.label ?? "")).length,
			checkedFields: result.checkedFields.length,
			receivers: result.receivers.length,
		};
	}

	function runFieldChange(path: FormFieldPath, value: string | boolean) {
		const change: FormChange = { path, value };
		const previous: FormState = {
			form: currentState.form,
			errors: currentState.errors,
		};

		clearTraceEvents();
		const firstResult = runVariant(pair[0], previous, change);
		const firstEvents = takeTraceEvents();
		const secondResult = runVariant(pair[1], previous, change);
		const allEvents = takeTraceEvents();
		const secondEvents = allEvents.slice(firstEvents.length);

		runs = [
			{
				result: firstResult,
				events: firstEvents,
				metrics: analyze(firstResult, firstEvents),
			},
			{
				result: secondResult,
				events: secondEvents,
				metrics: analyze(secondResult, secondEvents),
			},
		];
		currentState = { form: firstResult.form, errors: firstResult.errors };
		activePath = path;
		runCount++;
	}

	function resetSandbox() {
		clearTraceEvents();
		currentState = initialFormState();
		activePath = "shipping.address.postcode";
		runs = [];
		selectedIndex = 0;
		runCount = 0;
	}
</script>

<section class="min-w-0 overflow-hidden rounded-xl border bg-card" aria-labelledby="sandbox-heading">
	<div class="flex flex-wrap items-center gap-3 border-b bg-muted/20 px-3 py-2">
		<div class="flex items-center gap-2">
			<span class="flex gap-1" aria-hidden="true">
				<span class="size-2 rounded-full bg-red-400"></span>
				<span class="size-2 rounded-full bg-amber-400"></span>
				<span class="size-2 rounded-full bg-emerald-400"></span>
			</span>
			<h3 id="sandbox-heading" class="text-xs font-semibold">Form sandbox</h3>
		</div>
		<div class="ml-auto flex items-center gap-2 text-[10px] text-muted-foreground">
			<span class="flex items-center gap-1"><span class="size-1.5 rounded-full bg-emerald-500"></span>Live trace</span>
			{#if runCount > 0}<span>Update #{runCount}</span>{/if}
			<Button variant="ghost" size="icon-xs" aria-label="Reset form sandbox" onclick={resetSandbox}><RotateCcwIcon /></Button>
		</div>
	</div>

	<div class="flex flex-wrap items-end gap-3 border-b px-3 py-2.5">
		<fieldset class="min-w-60 flex-1">
			<legend class="mb-1 text-[9px] font-medium uppercase tracking-wide text-muted-foreground">Compare</legend>
			<div class="grid grid-cols-2 gap-1 rounded-md bg-muted/60 p-0.5">
				<button
					type="button"
					class={{
						"rounded px-2 py-1.5 text-[10px] transition-colors": true,
						"bg-background font-medium shadow-sm": compareAxis === "structure",
						"text-muted-foreground": compareAxis !== "structure",
					}}
					aria-pressed={compareAxis === "structure"}
					onclick={() => selectAxis("structure")}
				>
					Nested ↔ Flat
				</button>
				<button
					type="button"
					class={{
						"rounded px-2 py-1.5 text-[10px] transition-colors": true,
						"bg-background font-medium shadow-sm": compareAxis === "channel",
						"text-muted-foreground": compareAxis !== "channel",
					}}
					aria-pressed={compareAxis === "channel"}
					onclick={() => selectAxis("channel")}
				>
					Binding ↔ Store
				</button>
			</div>
		</fieldset>

		<fieldset class="min-w-48 flex-1">
			<legend class="mb-1 text-[9px] font-medium uppercase tracking-wide text-muted-foreground">
				Hold {compareAxis === "structure" ? "channel" : "structure"} constant
			</legend>
			{#if compareAxis === "structure"}
				<div class="grid grid-cols-2 gap-1 rounded-md bg-muted/60 p-0.5">
					{#each ["binding", "store"] as channel (channel)}
						<button
							type="button"
							class={{
								"rounded px-2 py-1.5 text-[10px] capitalize transition-colors": true,
								"bg-background font-medium shadow-sm": fixedChannel === channel,
								"text-muted-foreground": fixedChannel !== channel,
							}}
							aria-pressed={fixedChannel === channel}
							onclick={() => selectChannel(channel as StateChannel)}
						>
							{channel}
						</button>
					{/each}
				</div>
			{:else}
				<div class="grid grid-cols-2 gap-1 rounded-md bg-muted/60 p-0.5">
					{#each ["nested", "flat"] as structure (structure)}
						<button
							type="button"
							class={{
								"rounded px-2 py-1.5 text-[10px] capitalize transition-colors": true,
								"bg-background font-medium shadow-sm": fixedStructure === structure,
								"text-muted-foreground": fixedStructure !== structure,
							}}
							aria-pressed={fixedStructure === structure}
							onclick={() => selectStructure(structure as FormStructure)}
						>
							{structure}
						</button>
					{/each}
				</div>
			{/if}
		</fieldset>

		<div class="min-w-72 flex-[1.4] text-[10px] text-muted-foreground">
			Edit any field in Preview. The same change runs through
			<span class="font-medium text-foreground">A</span> and
			<span class="font-medium text-foreground">B</span> from the same previous state.
		</div>
	</div>

	<div class="grid min-w-0 grid-cols-2">
		<div class="min-w-0 border-r">
			{#key selectedVariant}
				<SourceCode
					source={scenarioSource}
					filename="form-validation/scenario.ts"
					focus={sourceFocus}
					embedded
					viewportClass="h-[720px]"
				/>
			{/key}
		</div>

		<div class="min-w-0">
			<div class="flex h-[45px] items-center justify-between gap-2 border-b px-3">
				<div class="flex items-center gap-2 text-xs font-medium"><EyeIcon class="size-3.5" />Preview</div>
				<div class="flex items-center gap-1">
					{#each pair as variant, index (variant)}
						<button
							type="button"
							class={{
								"rounded-md px-2 py-1 text-[10px] transition-colors": true,
								"bg-foreground font-medium text-background": selectedIndex === index,
								"bg-muted text-muted-foreground": selectedIndex !== index,
							}}
							onclick={() => (selectedIndex = index)}
							aria-pressed={selectedIndex === index}
						>
							{index === 0 ? "A" : "B"}<span class="hidden xl:inline"> · {variantMeta[variant].title}</span>
						</button>
					{/each}
				</div>
			</div>
			<div class="h-[720px] overflow-y-auto bg-muted/20">
				<FormPreview
					form={currentState.form}
					errors={currentState.errors}
					{activePath}
					{submitEnabled}
					onfieldchange={runFieldChange}
				/>
			</div>
		</div>
	</div>
</section>

<section class="min-w-0 rounded-xl border bg-card p-3" aria-labelledby="comparison-results-heading">
	<div class="mb-3 flex flex-wrap items-center justify-between gap-2">
		<div>
			<h3 id="comparison-results-heading" class="text-sm font-medium">Last field update</h3>
			<p class="pt-0.5 font-mono text-[10px] text-muted-foreground">{activePath}</p>
		</div>
		{#if runs.length === 2}
			<span
				class={{
					"inline-flex items-center gap-1 rounded-full px-2 py-1 text-[10px] font-medium": true,
					"bg-emerald-500/10 text-emerald-700 dark:text-emerald-300": outputsMatch,
					"bg-destructive/10 text-destructive": !outputsMatch,
				}}
			>
				{#if outputsMatch}<CheckIcon class="size-3" />Same form and errors{:else}<XIcon class="size-3" />Output mismatch{/if}
			</span>
		{/if}
	</div>

	{#if runs.length === 2}
		<div class="grid gap-3 md:grid-cols-2">
			{#each runs as run, index (run.result.variant)}
				<button
					type="button"
					class={{
						"min-w-0 rounded-lg border p-3 text-left transition-colors focus-visible:outline-2 focus-visible:outline-ring": true,
						"border-chart-2 bg-chart-2/5 ring-1 ring-chart-2/20": selectedIndex === index,
						"hover:bg-muted/40": selectedIndex !== index,
					}}
					aria-pressed={selectedIndex === index}
					onclick={() => (selectedIndex = index)}
				>
					<div class="flex items-start justify-between gap-2">
						<div>
							<div class="text-xs font-semibold">{index === 0 ? "A" : "B"} · {variantMeta[run.result.variant].title}</div>
							<div class="mt-1 flex flex-wrap items-center gap-1 text-[10px] text-muted-foreground">
								{#each variantMeta[run.result.variant].topology as step, stepIndex (step)}
									<span>{step}</span>
									{#if stepIndex < variantMeta[run.result.variant].topology.length - 1}<ArrowRightIcon class="size-2.5" />{/if}
								{/each}
							</div>
						</div>
						<span class="rounded-md bg-muted px-1.5 py-1 text-[10px]">
							{Object.keys(run.result.errors).length} errors
						</span>
					</div>

					<div class="mt-3 grid grid-cols-4 gap-1.5">
						{#each [
							["Calls", run.metrics.calls],
							["Writes", run.metrics.writes],
							["Checked", run.metrics.checkedFields],
							["Receivers", run.metrics.receivers],
						] as metric (metric[0])}
							<div class="rounded-md bg-muted/60 px-2 py-1.5 text-center">
								<div class="font-mono text-sm font-semibold tabular-nums">{metric[1]}</div>
								<div class="text-[9px] text-muted-foreground">{metric[0]}</div>
							</div>
						{/each}
					</div>

					<div class="mt-3 flex flex-wrap items-center justify-between gap-2 text-[10px]">
						<span class={run.result.errors[activePath] ? "text-destructive" : "text-emerald-700 dark:text-emerald-300"}>
							{run.result.errors[activePath] ?? "Changed field valid"}
						</span>
						<span class="text-muted-foreground">{run.result.receivers.join(" · ")}</span>
					</div>
				</button>
			{/each}
		</div>
		<p class="mt-3 rounded-lg bg-muted/40 px-3 py-2 text-xs text-muted-foreground">{finding}</p>
	{:else}
		<div class="flex min-h-24 items-center justify-center rounded-lg border border-dashed px-4 text-center text-xs text-muted-foreground">
			Edit a field in the Preview pane to run both implementations.
		</div>
	{/if}
</section>

<div class="grid min-w-0 grid-cols-1 items-start gap-3">
	<div class="min-w-0 rounded-xl border bg-card p-3">
		<div class="mb-2 flex flex-wrap items-center justify-between gap-2">
			<div class="text-[10px] text-muted-foreground">
				{#if selectedRun}
					Inspecting {selectedIndex === 0 ? "A" : "B"} · {variantMeta[selectedRun.result.variant].title}
				{:else}
					The selected implementation's trace appears after a field edit.
				{/if}
			</div>
			{#if runs.length === 2}
				<div class="flex gap-1 rounded-md bg-muted/60 p-0.5" aria-label="Trace variant">
					{#each runs as run, index (run.result.variant)}
						<button
							type="button"
							class={{
								"rounded px-2 py-1 text-[10px] transition-colors": true,
								"bg-background font-medium shadow-sm": selectedIndex === index,
								"text-muted-foreground": selectedIndex !== index,
							}}
							onclick={() => (selectedIndex = index)}
							aria-pressed={selectedIndex === index}
						>
							{index === 0 ? "A" : "B"}
						</button>
					{/each}
				</div>
			{/if}
		</div>
		<TracePanel
			events={selectedRun?.events ?? []}
			emptyHint="Edit a form field to generate a trace."
			class="h-[460px]"
		/>
	</div>

	<div class="flex min-w-0 flex-col gap-3 rounded-xl border bg-card p-3">
		<div class="flex flex-wrap items-start justify-between gap-2">
			<div>
				<h3 class="text-sm font-medium">Metrics</h3>
				<p class="pt-0.5 text-[10px] text-muted-foreground">Work performed by the latest field update</p>
			</div>
			{#if runs.length === 2}
				<div class="flex flex-wrap gap-3 text-[10px] text-muted-foreground">
					<span class="flex items-center gap-1"><span class="size-2 rounded-sm bg-chart-2"></span>A · {variantMeta[runs[0].result.variant].title}</span>
					<span class="flex items-center gap-1"><span class="size-2 rounded-sm bg-chart-3"></span>B · {variantMeta[runs[1].result.variant].title}</span>
				</div>
			{/if}
		</div>

		{#if metricRows.length > 0}
			<div class="overflow-x-auto">
				<div class="min-w-[600px]">
					<BarChart
						data={metricRows}
						x="label"
						series={[
							{ key: "left", label: "A", color: "var(--chart-2)" },
							{ key: "right", label: "B", color: "var(--chart-3)" },
						]}
						seriesLayout="group"
						yDomain={[0, null]}
						yNice={false}
						height={250}
						tooltip={false}
						tooltipContext={false}
						padding={{ top: 20, right: 16, bottom: 32, left: 40 }}
						props={{
							xAxis: { ticks: metricRows.map((row) => row.label) },
							yAxis: { ticks: metricTicks, format: (value: number) => value.toLocaleString() },
							bars: { strokeWidth: 0 },
						}}
					/>
				</div>
			</div>
		{:else}
			<div class="flex min-h-40 items-center justify-center text-xs text-muted-foreground">
				Edit a form field to compare runtime work.
			</div>
		{/if}

		<p class="border-t pt-3 text-[10px] leading-relaxed text-muted-foreground">
			The sandbox executes four scenario adapters. Native Svelte
			<code class="rounded bg-muted px-1 py-0.5">bind:value</code> and store subscriptions are represented by named boundaries until the transformer reads component files directly.
		</p>
	</div>
</div>

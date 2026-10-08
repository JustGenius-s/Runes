<script lang="ts">
	import EyeIcon from "@lucide/svelte/icons/eye";
	import RotateCcwIcon from "@lucide/svelte/icons/rotate-ccw";
	import { Button } from "$lib/components/ui/button/index.js";
	import {
		clearTraceEvents,
		takeTraceEvents,
		type TraceEvent,
	} from "virtual:runes-runtime";
	import SourceCode from "../../source-code.svelte";
	import TracePanel from "../../trace/trace-panel.svelte";
	import { buildDataPaths } from "../../trace/trace-paths.js";
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

	interface VariantRun {
		result: FormScenarioResult;
		events: TraceEvent[];
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
	// Both panels share one time axis so their bars compare directly.
	const sharedScaleNs = $derived(
		Math.max(0, ...runs.flatMap((run) => buildDataPaths(run.events).map((path) => path.totalNs))),
	);
	const selectedVariant = $derived(selectedRun?.result.variant ?? pair[selectedIndex]);
	const sourceFocus = $derived(sourceFocusByVariant[selectedVariant]);
	const submitEnabled = $derived(
		selectedRun?.result.submitEnabled ?? Object.keys(currentState.errors).length === 0,
	);
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
			{ result: firstResult, events: firstEvents },
			{ result: secondResult, events: secondEvents },
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

<section class="grid min-w-0 gap-4 rounded-xl border bg-card p-3 lg:grid-cols-2" aria-label="Data paths of the last field update">
	{#each pair as variant, index (variant)}
		<TracePanel
			events={runs[index]?.events ?? []}
			scaleNs={sharedScaleNs}
			title="{index === 0 ? 'A' : 'B'} · {variantMeta[variant].title}"
			emptyHint="Edit a field in Preview to trace both implementations."
		/>
	{/each}
</section>

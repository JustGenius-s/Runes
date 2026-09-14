<script lang="ts">
	import ArrowRightIcon from "@lucide/svelte/icons/arrow-right";
	import { catalog } from "./catalog.js";
	import { nav } from "./nav.svelte.js";
</script>

<div class="mx-auto flex w-full max-w-4xl flex-col gap-8 py-8">
	<div class="flex flex-col gap-2">
		<h1 class="text-3xl font-bold tracking-tight">Runes Explorer</h1>
		<p class="max-w-2xl text-muted-foreground">
			An explorer for front-end algorithms, design patterns, and component
			best practices. Runes traces the data flow of every experiment, turning
			"which implementation is better" into a measurable comparison.
		</p>
	</div>

	<div class="grid gap-4 md:grid-cols-3">
		{#each catalog as category (category.id)}
			<div class="flex flex-col rounded-xl border bg-card p-4 text-card-foreground">
				<div class="flex items-center gap-2 pb-3">
					<div
						class="flex size-8 items-center justify-center rounded-lg bg-primary text-primary-foreground"
					>
						<category.icon class="size-4" />
					</div>
					<h2 class="font-semibold">{category.title}</h2>
				</div>
				<div class="flex flex-col gap-1">
					{#each category.experiments as experiment (experiment.id)}
						<button
							class="group flex items-center justify-between rounded-md px-2 py-1.5 text-start text-sm text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
							onclick={() => nav.select(experiment.id)}
						>
							<span>{experiment.title}</span>
							<ArrowRightIcon
								class="size-3.5 opacity-0 transition-opacity group-hover:opacity-100"
							/>
						</button>
					{/each}
				</div>
			</div>
		{/each}
	</div>
</div>

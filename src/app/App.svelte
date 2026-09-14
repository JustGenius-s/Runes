<script lang="ts">
	import * as Breadcrumb from "$lib/components/ui/breadcrumb/index.js";
	import { Separator } from "$lib/components/ui/separator/index.js";
	import * as Sidebar from "$lib/components/ui/sidebar/index.js";
	import AppSidebar from "$lib/components/app-sidebar.svelte";
	import ExperimentView from "$lib/explorer/experiment-view.svelte";
	import HomeView from "$lib/explorer/home-view.svelte";
	import { nav } from "$lib/explorer/nav.svelte.js";
</script>

<Sidebar.Provider>
	<AppSidebar />
	<Sidebar.Inset>
		<header
			class="flex h-16 shrink-0 items-center gap-2 transition-[width,height] ease-linear group-has-data-[collapsible=icon]/sidebar-wrapper:h-12"
		>
			<div class="flex items-center gap-2 px-4">
				<Sidebar.Trigger class="-ms-1" />
				<Separator
					orientation="vertical"
					class="me-2 data-[orientation=vertical]:h-4"
				/>
				<Breadcrumb.Root>
					<Breadcrumb.List>
						<Breadcrumb.Item>
							{#if nav.selected}
								<Breadcrumb.Link onclick={() => nav.select(null)}>
									Runes Explorer
								</Breadcrumb.Link>
							{:else}
								<Breadcrumb.Page>Runes Explorer</Breadcrumb.Page>
							{/if}
						</Breadcrumb.Item>
						{#if nav.selected}
							<Breadcrumb.Separator />
							<Breadcrumb.Item class="hidden md:block">
								<span class="text-muted-foreground">
									{nav.selected.category.title}
								</span>
							</Breadcrumb.Item>
							<Breadcrumb.Separator class="hidden md:block" />
							<Breadcrumb.Item>
								<Breadcrumb.Page>
									{nav.selected.experiment.title}
								</Breadcrumb.Page>
							</Breadcrumb.Item>
						{/if}
					</Breadcrumb.List>
				</Breadcrumb.Root>
			</div>
		</header>
		<main class="flex flex-1 flex-col gap-4 p-4 pt-0">
			{#if nav.selected}
				{#key nav.selectedId}
					<ExperimentView selection={nav.selected} />
				{/key}
			{:else}
				<HomeView />
			{/if}
		</main>
	</Sidebar.Inset>
</Sidebar.Provider>

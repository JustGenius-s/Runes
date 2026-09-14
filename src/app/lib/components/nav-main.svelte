<script lang="ts">
	import ChevronRightIcon from "@lucide/svelte/icons/chevron-right";
	import * as Collapsible from "$lib/components/ui/collapsible/index.js";
	import * as Sidebar from "$lib/components/ui/sidebar/index.js";
	import type { Category } from "$lib/explorer/catalog.js";
	import { nav } from "$lib/explorer/nav.svelte.js";

	let { items }: { items: Category[] } = $props();
</script>

<Sidebar.Group>
	<Sidebar.GroupLabel>探索器</Sidebar.GroupLabel>
	<Sidebar.Menu>
		{#each items as category (category.id)}
			<Collapsible.Root open class="group/collapsible">
				{#snippet child({ props })}
					<Sidebar.MenuItem {...props}>
						<Collapsible.Trigger>
							{#snippet child({ props })}
								<Sidebar.MenuButton {...props} tooltipContent={category.title}>
									<category.icon />
									<span>{category.title}</span>
									<ChevronRightIcon
										class="ms-auto transition-transform duration-200 group-data-[state=open]/collapsible:rotate-90"
									/>
								</Sidebar.MenuButton>
							{/snippet}
						</Collapsible.Trigger>
						<Collapsible.Content>
							<Sidebar.MenuSub>
								{#each category.experiments as experiment (experiment.id)}
									<Sidebar.MenuSubItem>
										<Sidebar.MenuSubButton
											isActive={nav.selectedId === experiment.id}
											onclick={() => nav.select(experiment.id)}
										>
											<span>{experiment.title}</span>
										</Sidebar.MenuSubButton>
									</Sidebar.MenuSubItem>
								{/each}
							</Sidebar.MenuSub>
						</Collapsible.Content>
					</Sidebar.MenuItem>
				{/snippet}
			</Collapsible.Root>
		{/each}
	</Sidebar.Menu>
</Sidebar.Group>

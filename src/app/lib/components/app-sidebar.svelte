<script lang="ts">
	import SparklesIcon from "@lucide/svelte/icons/sparkles";
	import * as Sidebar from "$lib/components/ui/sidebar/index.js";
	import { catalog } from "$lib/explorer/catalog.js";
	import { nav } from "$lib/explorer/nav.svelte.js";
	import NavMain from "./nav-main.svelte";
	import NavUser from "./nav-user.svelte";
	import type { ComponentProps } from "svelte";

	const user = {
		name: "Runes",
		email: "explorer@runes.local",
		avatar: "",
	};

	let {
		ref = $bindable(null),
		collapsible = "icon",
		...restProps
	}: ComponentProps<typeof Sidebar.Root> = $props();
</script>

<Sidebar.Root bind:ref {collapsible} {...restProps}>
	<Sidebar.Header>
		<Sidebar.Menu>
			<Sidebar.MenuItem>
				<Sidebar.MenuButton size="lg" onclick={() => nav.select(null)}>
					<div
						class="flex aspect-square size-8 items-center justify-center rounded-lg bg-sidebar-primary text-sidebar-primary-foreground"
					>
						<SparklesIcon class="size-4" />
					</div>
					<div class="grid flex-1 text-start text-sm leading-tight">
						<span class="truncate font-semibold">Runes Explorer</span>
						<span class="truncate text-xs text-muted-foreground">
							Data-flow analysis
						</span>
					</div>
				</Sidebar.MenuButton>
			</Sidebar.MenuItem>
		</Sidebar.Menu>
	</Sidebar.Header>
	<Sidebar.Content>
		<NavMain items={catalog} />
	</Sidebar.Content>
	<Sidebar.Footer>
		<NavUser {user} />
	</Sidebar.Footer>
	<Sidebar.Rail />
</Sidebar.Root>

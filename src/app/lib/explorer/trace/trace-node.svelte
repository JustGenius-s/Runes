<script lang="ts">
	import {
		Handle,
		Position,
		type Node,
		type NodeProps,
	} from "@xyflow/svelte";

	export type TraceNodeData = {
		label: string;
		kind: "root" | "derive";
		value?: string;
	};

	let { data }: NodeProps<Node<TraceNodeData>> = $props();
</script>

{#if data.kind !== "root"}
	<Handle type="target" position={Position.Left} />
{/if}
<div
	class={{
		"min-w-24 rounded-lg border bg-card px-3 py-2 shadow-sm": true,
		"border-chart-1/70": data.kind === "root",
		"border-chart-4/70": data.kind === "derive",
	}}
>
	<div class="font-mono text-xs font-medium">{data.label}</div>
	{#if data.value !== undefined}
		<div
			class="max-w-44 truncate font-mono text-[10px] text-muted-foreground"
			title={data.value}
		>
			= {data.value}
		</div>
	{/if}
</div>
<Handle type="source" position={Position.Right} />

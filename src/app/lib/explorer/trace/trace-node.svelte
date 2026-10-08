<script lang="ts">
	import {
		Handle,
		Position,
		type Node,
		type NodeProps,
	} from "@xyflow/svelte";

	export type TraceNodeData = {
		label: string;
		kind: "root" | "derive" | "call";
		output?: string;
		value?: string;
		detail?: string;
	};

	let { data }: NodeProps<Node<TraceNodeData>> = $props();
</script>

{#if data.kind !== "root"}
	<Handle type="target" position={Position.Left} />
{/if}
<div
	class={{
		"min-w-32 rounded-lg border bg-card px-3 py-2 shadow-sm": true,
		"border-chart-1/70": data.kind === "root",
		"border-chart-4/70": data.kind === "derive",
		"border-chart-2/70": data.kind === "call",
	}}
>
	<div class="mb-1 text-[9px] font-semibold uppercase tracking-wider text-muted-foreground">
		{data.kind === "root" ? "Input" : data.kind === "call" ? "Call" : "Value"}
	</div>
	<div class="font-mono text-xs font-semibold">{data.label}</div>
	{#if data.output}
		<div class="mt-1 border-t pt-1 font-mono text-[10px]">
			<span class="text-muted-foreground">produces</span>
			<span class="ml-1">{data.output}</span>
		</div>
	{/if}
	{#if data.value !== undefined}
		<div
			class="max-w-48 truncate font-mono text-[10px] text-muted-foreground"
			title={data.value}
		>
			= {data.value}
		</div>
	{/if}
	{#if data.detail}
		<div class="mt-1 text-[9px] text-muted-foreground">{data.detail}</div>
	{/if}
</div>
<Handle type="source" position={Position.Right} />

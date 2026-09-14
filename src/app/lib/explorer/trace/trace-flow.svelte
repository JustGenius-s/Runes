<script lang="ts">
	import {
		Background,
		Controls,
		MarkerType,
		SvelteFlow,
		type Edge,
		type Node,
	} from "@xyflow/svelte";
	import "@xyflow/svelte/dist/style.css";
	import type { TraceEvent } from "virtual:runes-runtime";
	import TraceNode, { type TraceNodeData } from "./trace-node.svelte";

	type FlowNode = Node<TraceNodeData>;

	interface Props {
		events: TraceEvent[];
		values?: Record<string, string>;
		emptyHint?: string;
	}

	let {
		events,
		values = {},
		emptyHint = "No trace events yet.",
	}: Props = $props();

	const nodeTypes = { trace: TraceNode };

	let nodes = $state.raw<FlowNode[]>([]);
	let edges = $state.raw<Edge[]>([]);

	$effect(() => {
		const built = buildGraph(events, values);
		nodes = built.nodes;
		edges = built.edges;
	});

	// Reconstructs the derivation chain from one action's events: a root
	// node, then one node per value_derive, linked by the call that
	// produced it (the call_enter seen just before the derive).
	function buildGraph(
		events: TraceEvent[],
		values: Record<string, string>,
	): { nodes: FlowNode[]; edges: Edge[] } {
		const nodes: FlowNode[] = [];
		const edges: Edge[] = [];
		let pendingCall: string | null = null;
		for (const event of events) {
			if (event.event === "root" && event.binding) {
				nodes.push(makeNode(event.binding, "root", nodes.length, values));
			} else if (event.event === "call_enter") {
				pendingCall = event.label ?? null;
			} else if (event.event === "value_derive" && event.label) {
				const source = nodes[nodes.length - 1];
				nodes.push(makeNode(event.label, "derive", nodes.length, values));
				if (source) {
					edges.push({
						id: source.id + "->" + event.label,
						source: source.id,
						target: event.label,
						label: pendingCall ? pendingCall + "()" : undefined,
						type: "smoothstep",
						animated: true,
						markerEnd: { type: MarkerType.ArrowClosed },
					});
				}
				pendingCall = null;
			}
		}
		return { nodes, edges };
	}

	function makeNode(
		id: string,
		kind: "root" | "derive",
		index: number,
		values: Record<string, string>,
	): FlowNode {
		return {
			id,
			type: "trace",
			position: { x: index * 200, y: 40 },
			data: { label: id, kind, value: values[id] },
		};
	}
</script>

{#if nodes.length === 0}
	<div
		class="flex h-full items-center justify-center text-xs text-muted-foreground"
	>
		{emptyHint}
	</div>
{:else}
	<SvelteFlow
		bind:nodes
		bind:edges
		{nodeTypes}
		fitView
		fitViewOptions={{ padding: 0.3 }}
		colorMode="system"
		nodesConnectable={false}
		deleteKey={null}
	>
		<Background />
		<Controls showInteractive={false} />
	</SvelteFlow>
{/if}

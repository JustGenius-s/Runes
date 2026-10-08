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
	let callCount = $state(0);

	$effect(() => {
		const built = buildGraph(events, values);
		nodes = built.nodes;
		edges = built.edges;
		callCount = built.callCount;
	});

	function buildGraph(
		events: TraceEvent[],
		fallbackValues: Record<string, string>,
	): { nodes: FlowNode[]; edges: Edge[]; callCount: number } {
		const graphNodes: FlowNode[] = [];
		const graphEdges: Edge[] = [];
		const levels = new Map<string, number>();
		const bindingNodes = new Map<string, string>();
		const callNodes = new Map<number, string>();
		const handledDerives = new Set<number>();
		const edgeIds = new Set<string>();
		const exits = new Map(
			events
				.filter((event) => event.event === "call_exit" && event.call_id !== undefined)
				.map((event) => [event.call_id!, event]),
		);
		const derivesByCall = new Map<number, TraceEvent>();
		for (const event of events) {
			if (event.event !== "value_derive") continue;
			for (const callId of event.producer_call_ids ?? []) derivesByCall.set(callId, event);
		}

		function dependencyNodes(dependencies: string[] | undefined): string[] {
			return [...new Set((dependencies ?? []).map((name) => bindingNodes.get(name)).filter(Boolean))] as string[];
		}

		function addEdge(source: string, target: string) {
			const id = `${source}->${target}`;
			if (edgeIds.has(id)) return;
			edgeIds.add(id);
			graphEdges.push({
				id,
				source,
				target,
				type: "smoothstep",
				markerEnd: { type: MarkerType.ArrowClosed },
				style: "stroke-width: 1.5",
			});
		}

		function addNode(id: string, data: TraceNodeData, sources: string[]) {
			const level = sources.length
				? Math.max(...sources.map((source) => levels.get(source) ?? 0)) + 1
				: 0;
			levels.set(id, level);
			graphNodes.push({ id, type: "trace", position: { x: 0, y: 0 }, data });
			for (const source of sources) addEdge(source, id);
		}

		for (const [index, event] of events.entries()) {
			if (event.event === "root" && event.binding) {
				const id = `root-${event.value_id ?? index}`;
				addNode(id, {
					label: event.binding,
					kind: "root",
					value: event.value_preview ?? fallbackValues[event.binding],
				}, []);
				bindingNodes.set(event.binding, id);
				continue;
			}

			if (event.event === "call_enter" && event.call_id !== undefined) {
				const id = `call-${event.call_id}`;
				const produced = derivesByCall.get(event.call_id);
				const exit = exits.get(event.call_id);
				const sources = dependencyNodes(event.dependencies);
				if (sources.length === 0 && event.parent_call_id !== undefined) {
					const parent = callNodes.get(event.parent_call_id);
					if (parent) sources.push(parent);
				}
				addNode(id, {
					label: `${event.label ?? "unknown"}()`,
					kind: "call",
					output: produced?.label,
					value: produced?.value_preview ?? exit?.result_preview,
					detail: exit?.unwind ? "threw" : undefined,
				}, sources);
				callNodes.set(event.call_id, id);
				if (produced?.event === "value_derive" && produced.value_id !== undefined) {
					handledDerives.add(produced.value_id);
					if (produced.label) bindingNodes.set(produced.label, id);
				}
				continue;
			}

			if (event.event === "value_derive" && event.label) {
				if (event.value_id !== undefined && handledDerives.has(event.value_id)) continue;
				const producerNodes = (event.producer_call_ids ?? [])
					.map((callId) => callNodes.get(callId))
					.filter(Boolean) as string[];
				const sources = producerNodes.length > 0 ? producerNodes : dependencyNodes(event.dependencies);
				const id = `value-${event.value_id ?? index}`;
				addNode(id, {
					label: event.label,
					kind: "derive",
					value: event.value_preview ?? fallbackValues[event.label],
				}, sources);
				bindingNodes.set(event.label, id);
			}
		}

		const columns = new Map<number, FlowNode[]>();
		for (const node of graphNodes) {
			const level = levels.get(node.id) ?? 0;
			const column = columns.get(level) ?? [];
			column.push(node);
			columns.set(level, column);
		}
		const maxRows = Math.max(1, ...columns.values().map((column) => column.length));
		for (const [level, column] of columns) {
			const offset = ((maxRows - column.length) * 140) / 2;
			for (const [row, node] of column.entries()) {
				node.position = { x: 32 + level * 230, y: 56 + offset + row * 140 };
			}
		}

		return {
			nodes: graphNodes,
			edges: graphEdges,
			callCount: events.filter((event) => event.event === "call_enter").length,
		};
	}
</script>

{#if nodes.length === 0}
	<div class="flex h-full items-center justify-center text-xs text-muted-foreground">
		{emptyHint}
	</div>
{:else}
	<div class="relative h-full">
		<div class="pointer-events-none absolute left-3 top-3 z-10 rounded-md border bg-background/90 px-2.5 py-1.5 text-[10px] text-muted-foreground shadow-sm backdrop-blur">
			Latest action · {callCount} {callCount === 1 ? "call" : "calls"} · arrows show direct inputs
		</div>
		<SvelteFlow
			bind:nodes
			bind:edges
			{nodeTypes}
			fitView
			fitViewOptions={{ padding: 0.1 }}
			colorMode="light"
			nodesConnectable={false}
			deleteKey={null}
			minZoom={0.35}
			maxZoom={1.25}
		>
			<Background gap={24} size={1} />
			<Controls showInteractive={false} />
		</SvelteFlow>
	</div>
{/if}

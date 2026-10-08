import type { Component } from "svelte";
import ComponentIcon from "@lucide/svelte/icons/component";
import PuzzleIcon from "@lucide/svelte/icons/puzzle";
import SigmaIcon from "@lucide/svelte/icons/sigma";

/** A single traceable experiment. */
export interface Experiment {
	/** Globally unique id, used for nav selection. */
	id: string;
	title: string;
	/** What this experiment quantifies. */
	summary: string;
}

/** A category in the sidebar. */
export interface Category {
	id: string;
	title: string;
	icon: Component;
	experiments: Experiment[];
}

export const catalog: Category[] = [
	{
		id: "algorithms",
		title: "Algorithms",
		icon: SigmaIcon,
		experiments: [
			{
				id: "sorting",
				title: "Sorting",
				summary:
					"Bubble vs quick vs merge: comparisons, writes, and derivation chain length.",
			},
			{
				id: "binary-search",
				title: "Binary Search",
				summary: "Interval convergence path and access sequence per step.",
			},
			{
				id: "graph-traversal",
				title: "Graph Traversal",
				summary: "BFS vs DFS visit order with live queue/stack changes.",
			},
			{
				id: "dp-fib",
				title: "Dynamic Programming",
				summary:
					"Fibonacci: naive recursion vs memoization redundant-computation comparison.",
			},
		],
	},
	{
		id: "patterns",
		title: "Design Patterns",
		icon: PuzzleIcon,
		experiments: [
			{
				id: "pub-sub",
				title: "Pub/Sub",
				summary:
					"Subscriber notification chain and fan-out width per published event.",
			},
			{
				id: "strategy",
				title: "Strategy",
				summary:
					"Call distribution and context dependencies after runtime strategy switches.",
			},
			{
				id: "state-machine",
				title: "State Machine",
				summary:
					"Transition paths, transition counts, and intercepted illegal transitions.",
			},
			{
				id: "memento",
				title: "Memento",
				summary: "Undo/redo snapshot count, size, and restore paths.",
			},
		],
	},
	{
		id: "components",
		title: "Components",
		icon: ComponentIcon,
		experiments: [
			{
				id: "counter",
				title: "Counter",
				summary:
					"The minimal state unit: derivation updates triggered by a single click.",
			},
			{
				id: "todo-list",
				title: "Todo List",
				summary:
					"Derived computation scope caused by add/remove/toggle and filtering.",
			},
			{
				id: "form-validation",
				title: "Form Validation",
				summary:
					"Nested vs flat composition and binding vs store updates across a multi-section form.",
			},
			{
				id: "store-compare",
				title: "Store Comparison",
				summary:
					"Update fan-out of different state-management approaches on the same scene.",
			},
		],
	},
];

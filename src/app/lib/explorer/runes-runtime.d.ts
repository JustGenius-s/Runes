/** Types for the Runes virtual runtime module served by the vite plugin. */
declare module "virtual:runes-runtime" {
	export interface TraceEvent {
		event: "root" | "call_enter" | "call_exit" | "value_derive";
		at_ns: number;
		label?: string;
		binding?: string;
		call_id?: number;
		site_id?: string;
		parent_call_id?: number;
		value_id?: number;
		roots?: string[];
		dependencies?: string[];
		producer_call_ids?: number[];
		value_preview?: string;
		result_preview?: string;
		duration_ns?: number;
		unwind?: boolean;
		file?: string;
		line?: number;
		column?: number;
	}
	export function takeTraceEvents(): TraceEvent[];
	export function clearTraceEvents(): void;
	export function subscribeTrace(listener: (event: TraceEvent) => void): () => void;
	export function overflowedEventCount(): number;
}

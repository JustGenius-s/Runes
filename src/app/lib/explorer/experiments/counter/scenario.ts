// Counter experiment scenario. The rune keyword marks each step's input
// as a trace root; the Runes vite plugin instruments this file, so every
// derived binding and dependent call below emits trace events.

/** Result of one counter step, derived from the traced input. */
export interface CounterStep {
	next: number;
	doubled: number;
	parity: string;
	summary: string;
}

function increment(n: number): number {
	return n + 1;
}

function decrement(n: number): number {
	return n - 1;
}

function double(n: number): number {
	return n * 2;
}

function parityOf(n: number): string {
	return n % 2 === 0 ? "even" : "odd";
}

function summarize(n: number, parity: string): string {
	return n + " is " + parity;
}

/** Runs one instrumented counter step from current by delta. */
export function step(current: number, delta: 1 | -1): CounterStep {
	rune count = current;
	const next = delta === 1 ? increment(count) : decrement(count);
	const doubled = double(next);
	const parity = parityOf(next);
	const summary = summarize(next, parity);
	return { next, doubled, parity, summary };
}

<script lang="ts" module>
	import hljs from "highlight.js/lib/core";
	import typescript from "highlight.js/lib/languages/typescript";

	const highlighter = hljs.newInstance();
	highlighter.registerLanguage("runes-typescript", (api) => {
		const language = typescript(api);
		language.contains.unshift({ scope: "keyword", match: /\brune\b/ });
		return language;
	});
</script>

<script lang="ts">
	import CheckIcon from "@lucide/svelte/icons/check";
	import CodeIcon from "@lucide/svelte/icons/code";
	import CopyIcon from "@lucide/svelte/icons/copy";
	import { Button } from "$lib/components/ui/button/index.js";

	interface Props {
		source: string;
		filename: string;
		/** Optional entry point shown initially; line numbers stay relative to the file. */
		focus?: { label: string; startLine: number; endLine: number };
		/** Removes the outer divider when the source viewer is placed inside a playground pane. */
		embedded?: boolean;
		/** Controls the scrollable code viewport height. */
		viewportClass?: string;
	}

	let {
		source,
		filename,
		focus,
		embedded = false,
		viewportClass = "max-h-80",
	}: Props = $props();
	let showFullFile = $state(false);
	let copyStatus = $state<"idle" | "copied" | "error">("idle");

	const lines = $derived(source.trimEnd().split(/\r?\n/));
	const startLine = $derived(!showFullFile && focus ? focus.startLine : 1);
	const visibleLines = $derived(
		!showFullFile && focus ? lines.slice(focus.startLine - 1, focus.endLine) : lines,
	);
	const visibleSource = $derived(visibleLines.join("\n"));
	const highlighted = $derived(
		highlighter.highlight(visibleSource, { language: "runes-typescript" }).value,
	);

	$effect(() => {
		if (copyStatus !== "copied") return;
		const timeout = setTimeout(() => (copyStatus = "idle"), 2000);
		return () => clearTimeout(timeout);
	});

	function selectView(fullFile: boolean) {
		showFullFile = fullFile;
		copyStatus = "idle";
	}

	async function copyCode() {
		try {
			await navigator.clipboard.writeText(visibleSource);
			copyStatus = "copied";
		} catch {
			copyStatus = "error";
		}
	}
</script>

<section
	class="source-code min-w-0 {embedded ? '' : 'border-t'}"
	aria-label="Playground source code"
>
	<div class="flex flex-wrap items-center justify-between gap-2 px-4 py-2">
		<div class="flex min-w-0 items-center gap-2 text-xs text-muted-foreground">
			<CodeIcon class="size-3.5 shrink-0" />
			<span class="font-medium text-foreground">Code</span>
			<span class="truncate font-mono" title={filename}>{filename}</span>
		</div>
		<div class="flex items-center gap-2">
			{#if focus}
				<div class="flex gap-0.5 rounded-md bg-muted/60 p-0.5" role="group" aria-label="Code view">
					{#each [{ label: focus.label, fullFile: false }, { label: "Full file", fullFile: true }] as view (view.label)}
						<button
							type="button"
							class={{
								"rounded px-2 py-1 text-xs transition-colors focus-visible:outline-2 focus-visible:outline-ring": true,
								"bg-background font-medium shadow-sm": showFullFile === view.fullFile,
								"text-muted-foreground hover:text-foreground": showFullFile !== view.fullFile,
							}}
							aria-pressed={showFullFile === view.fullFile}
							onclick={() => selectView(view.fullFile)}
						>
							{view.label}
						</button>
					{/each}
				</div>
			{/if}
			<Button variant="ghost" size="xs" aria-label="Copy code" onclick={copyCode}>
				{#if copyStatus === "copied"}<CheckIcon />{:else}<CopyIcon />{/if}
				{copyStatus === "copied" ? "Copied" : "Copy"}
			</Button>
		</div>
	</div>
	{#key showFullFile}
		<!-- svelte-ignore a11y_no_noninteractive_tabindex (The scrollable code region needs keyboard focus for arrow-key scrolling.) -->
		<div class="{viewportClass} overflow-auto bg-muted/30 outline-offset-[-2px] focus-visible:outline-2 focus-visible:outline-ring" role="region" aria-label={filename} tabindex="0">
			<div class="flex min-w-max py-3 font-mono text-xs leading-6">
				<div class="sticky left-0 shrink-0 select-none border-r bg-card px-3 text-right text-muted-foreground" aria-hidden="true">
					{#each visibleLines as _, index (index)}
						<div>{startLine + index}</div>
					{/each}
				</div>
				<pre class="m-0 px-4 [tab-size:2]"><code class="language-typescript">{@html highlighted}</code></pre>
			</div>
		</div>
	{/key}
	<span class="sr-only" role="status">{copyStatus === "copied" ? "Code copied to clipboard." : ""}</span>
	{#if copyStatus === "error"}
		<p class="px-4 py-2 text-xs text-destructive" role="alert">Copy failed. Select the code and copy it manually.</p>
	{/if}
</section>

<style>
	.source-code {
		--code-keyword: #a626a4;
		--code-function: #2563a6;
		--code-string: #287c3c;
		--code-number: #a34c14;
	}

	:global(.dark) .source-code {
		--code-keyword: #d7a1e7;
		--code-function: #8fc7ff;
		--code-string: #a1d69b;
		--code-number: #efbc8a;
	}

	.source-code :global(.hljs-keyword),
	.source-code :global(.hljs-literal) {
		color: var(--code-keyword);
	}

	.source-code :global(.hljs-title),
	.source-code :global(.hljs-built_in),
	.source-code :global(.hljs-type) {
		color: var(--code-function);
	}

	.source-code :global(.hljs-string) {
		color: var(--code-string);
	}

	.source-code :global(.hljs-number) {
		color: var(--code-number);
	}

	.source-code :global(.hljs-comment),
	.source-code :global(.hljs-doctag) {
		color: var(--muted-foreground);
	}
</style>

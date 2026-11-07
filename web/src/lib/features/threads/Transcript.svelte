<script lang="ts">
	import { preClass, summaryClass } from '$lib/features/agents/disclosure';
	import { prettyJson, type TranscriptEntry } from '$lib/stores/transcript';
	import Markdown from '$lib/ui/Markdown.svelte';

	interface Props {
		entries: TranscriptEntry[];
	}

	let { entries }: Props = $props();

	const toolLabels: Record<string, string> = {
		'code-tool': 'Ran code'
	};

	function toolLabel(name: string): string {
		return toolLabels[name] ?? `Called ${name}`;
	}
</script>

{#snippet result(ok: boolean, text: string)}
	<details class="group">
		<summary class={summaryClass}>{ok ? 'Result' : 'Failed'}</summary>
		<pre class={[preClass, ok ? 'text-fg-muted' : 'text-danger']}>{prettyJson(text)}</pre>
	</details>
{/snippet}

<div class="flex flex-col gap-3">
	{#each entries as entry (entry.id)}
		{#if entry.type === 'user'}
			<section class="mt-3 border-t border-border pt-5 first:mt-0 first:border-t-0 first:pt-0">
				<div class="mb-1 text-xs font-medium text-fg-subtle">You</div>
				<p class="wrap-anywhere whitespace-pre-wrap">{entry.text}</p>
			</section>
		{:else if entry.type === 'assistant'}
			<div class="text-md"><Markdown source={entry.text} diagrams /></div>
		{:else if entry.type === 'thinking'}
			<details class="group">
				<summary class={summaryClass}>Thinking</summary>
				<pre class={[preClass, 'text-fg-muted']}>{entry.text ||
						(entry.redacted ? 'Redacted by the provider.' : '')}</pre>
			</details>
		{:else if entry.type === 'toolRequest'}
			<div class="flex flex-col gap-1">
				<details class="group">
					<summary class={summaryClass}>{toolLabel(entry.tool_name)}</summary>
					<pre class={[preClass, 'text-fg-muted']}>{entry.code ?? entry.arguments}</pre>
				</details>
				{#if entry.response}
					{@render result(entry.response.ok, entry.response.text)}
				{/if}
			</div>
		{:else}
			<div class="flex flex-col gap-1">{@render result(entry.ok, entry.text)}</div>
		{/if}
	{/each}
</div>

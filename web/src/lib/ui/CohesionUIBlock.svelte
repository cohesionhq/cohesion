<!--
	One CohesionUI block. A heading is a heading and a link a link here as
	on every client; an overview nested on its own is a list of one. A block
	adds no space around itself: `CohesionUI` spaces them, so each sits on
	the same left edge and the same rhythm.
-->
<script lang="ts">
	import type { Block } from '$lib/protocol/types';
	import CohesionUI from './CohesionUI.svelte';

	let { block }: { block: Block } = $props();

	/** Web addresses open in a new tab; a `mailto:` opens the mail app. */
	const external = (destination: string) => /^https?:/i.test(destination);
</script>

{#if block.kind === 'heading'}
	<!-- More space above than below, so a heading belongs to what follows it. -->
	{#if block.level === 1}
		<h1 class="mt-4 text-lg first:mt-0">{block.text}</h1>
	{:else if block.level === 2}
		<h2 class="mt-4 text-md first:mt-0">{block.text}</h2>
	{:else}
		<h3 class="mt-4 text-fg-muted first:mt-0">{block.text}</h3>
	{/if}
{:else if block.kind === 'text'}
	<p class="whitespace-pre-line">{block.text}</p>
{:else if block.kind === 'checkbox'}
	<p class="flex items-start gap-2">
		<!-- As tall as a line, so the box centres on the label's first line. -->
		<span class="flex h-lh flex-none items-center">
			<!-- Shows a value the user can't change here, so it takes no clicks or focus. -->
			<input
				class="pointer-events-none m-0 size-3.5 accent-accent"
				type="checkbox"
				checked={block.checked}
				tabindex="-1"
				aria-readonly="true"
				aria-label={block.text ? undefined : block.checked ? 'Yes' : 'No'}
			/>
		</span>
		{#if block.text}<span class="min-w-0">{block.text}</span>{/if}
	</p>
{:else if block.kind === 'link'}
	<p>
		<a
			href={block.destination}
			target={external(block.destination) ? '_blank' : undefined}
			rel="noopener noreferrer">{block.text}</a
		>
	</p>
{:else if block.kind === 'overview'}
	<CohesionUI component={{ blocks: [block] }} />
{/if}

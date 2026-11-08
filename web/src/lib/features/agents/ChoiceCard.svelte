<!--
	A question the agent asked with answers to pick from, inline in the feed:
	the question, then one row per answer with a radio's mark. Picking one starts the agent's turn on
	it; from then on the pick is marked as an actor's question card marks an
	answer, and the rest fade, none of them clickable.
-->
<script lang="ts">
	import Check from '@lucide/svelte/icons/check';
	import type { AgentChoice } from '$lib/protocol/types';
	import Markdown from '$lib/ui/Markdown.svelte';
	import { toasts } from '$lib/ui/toasts.svelte';

	interface Props {
		choice: AgentChoice;
		onpick: (option: number) => Promise<unknown>;
	}

	let { choice, onpick }: Props = $props();

	const id = $props.id();
	const pending = $derived(choice.selected === null);

	async function pick(option: number) {
		try {
			await onpick(option);
		} catch (error) {
			// The card offers its answers again.
			toasts.error(error);
		}
	}
</script>

<!-- Drawn as an actor's question card is: the same frame, rows and marks. -->
<article
	class="flex w-full min-w-0 flex-col gap-1.5 rounded-md border border-border bg-bg px-3 py-2.5"
	aria-labelledby="{id}-question"
>
	<div id="{id}-question">
		<Markdown source={choice.question} />
	</div>
	<ol class="flex flex-col gap-0.5">
		{#each choice.options as option, index (index)}
			{@const picked = choice.selected === index}
			<li>
				<button
					class={[
						'flex w-full items-start gap-2.5 rounded-sm px-2 py-1.25 text-left transition-colors',
						pending
							? 'cursor-pointer hover:bg-bg-hover'
							: picked
								? 'bg-accent-soft text-fg shadow-[inset_0_0_0_1px_color-mix(in_oklch,var(--accent)_40%,transparent)]'
								: 'text-fg-subtle'
					]}
					type="button"
					disabled={!pending}
					aria-pressed={picked}
					onclick={() => pick(index)}
				>
					<!-- A radio's mark, filled with a check once picked. -->
					<span
						class={[
							'mt-0.5 grid size-4 flex-none place-items-center rounded-full border transition-colors',
							picked
								? 'border-accent bg-accent text-accent-fg'
								: 'border-border-strong dark:border-fg-subtle'
						]}
					>
						{#if picked}<Check size={10} strokeWidth={3} aria-label="Picked" />{/if}
					</span>
					<span class="min-w-0 flex-1 wrap-anywhere">{option}</span>
				</button>
			</li>
		{/each}
	</ol>
</article>

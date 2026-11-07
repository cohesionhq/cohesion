<!--
	A question the agent asked with answers to pick from, inline in the feed:
	the question, then one button per answer, lettered A, B, C… Picking one
	starts the agent's turn on it; from then on the pick is marked with a check in
	an accent disc, as a reaction is drawn, and the rest fade, none of them
	clickable.
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

<article
	class="flex w-fit max-w-[70%] min-w-0 flex-col gap-1.5 rounded-md border border-border bg-bg p-2.5 max-sm:max-w-full"
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
						'flex w-full items-start gap-2 rounded-sm px-2 py-1.25 text-left transition-colors',
						pending
							? 'cursor-pointer bg-bg-raised hover:bg-bg-hover'
							: picked
								? 'bg-accent-soft text-fg'
								: 'text-fg-muted'
					]}
					type="button"
					disabled={!pending}
					aria-pressed={picked}
					onclick={() => pick(index)}
				>
					<!-- A key cap, a line tall so it centres on the answer's first line. -->
					<span class="flex h-lh flex-none items-center">
						<span
							class="inline-grid size-4.5 place-items-center rounded-[5px] bg-bg text-2xs font-semibold text-fg-muted shadow-[0_0_0_1px_var(--border)]"
							>{String.fromCharCode(65 + index)}</span
						>
					</span>
					<span class="min-w-0 flex-1 wrap-anywhere">{option}</span>
					{#if picked}
						<!-- A line tall, so the disc centres on the answer's first line; the
						     check is 0.6 of the disc, as a reaction's emoji is. -->
						<span class="flex h-lh flex-none items-center">
							<span class="grid size-5 place-items-center rounded-full bg-accent text-accent-fg">
								<Check size={12} strokeWidth={2.5} aria-label="Picked" />
							</span>
						</span>
					{/if}
				</button>
			</li>
		{/each}
	</ol>
</article>

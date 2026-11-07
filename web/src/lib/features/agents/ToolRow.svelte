<!-- One step the agent took, as a quiet line like the feed's other status lines: an icon and a title,
     opening in place onto `children` under a rule, as a fork's steps open, when it has them. -->
<script lang="ts">
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import type { Component, Snippet } from 'svelte';

	interface Props {
		title: string;
		/** The leading icon of a row with nothing to open; a row with `children` shows a chevron. */
		icon?: Component<{ class?: string; size?: number; strokeWidth?: number }>;
		/** Shown at the row's end, such as a failure. */
		trailing?: Snippet;
		children?: Snippet;
	}

	let { title, icon: Icon, trailing, children }: Props = $props();

	const box = 'max-w-[70%] min-w-0 max-sm:max-w-full';
	const row = 'flex items-center gap-1.5 text-xs leading-6 text-fg-subtle';
</script>

{#snippet label()}
	<span class="min-w-0 flex-1 truncate">{title}</span>
	{@render trailing?.()}
{/snippet}

{#if children}
	<details class={['group', box]}>
		<summary
			class={[
				row,
				'cursor-pointer select-none hover:text-fg-muted [&::-webkit-details-marker]:hidden'
			]}
		>
			<ChevronRight
				class="flex-none transition-transform group-open:rotate-90"
				size={12}
				strokeWidth={1.75}
			/>
			{@render label()}
		</summary>
		<div class="mt-1 ml-1.5 flex flex-col gap-2 border-l-2 border-border pl-3">
			{@render children()}
		</div>
	</details>
{:else}
	<p class={[box, row]}>
		{#if Icon}<Icon class="flex-none" size={12} strokeWidth={1.75} />{/if}
		{@render label()}
	</p>
{/if}

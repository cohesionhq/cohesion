<!-- One step the agent took, as a narrow card: an icon and a title, opening onto `children` when it has them. -->
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

	const card = 'max-w-[70%] min-w-0 rounded-md border border-border bg-bg max-sm:max-w-full';
	const row = 'flex items-center gap-2 px-3 py-1.5 text-xs text-fg-muted';
</script>

{#snippet label()}
	<span class="min-w-0 flex-1 truncate">{title}</span>
	{@render trailing?.()}
{/snippet}

{#if children}
	<details class={['group', card]}>
		<summary
			class={[row, 'cursor-pointer select-none hover:text-fg [&::-webkit-details-marker]:hidden']}
		>
			<ChevronRight
				class="flex-none transition-transform group-open:rotate-90"
				size={14}
				strokeWidth={1.75}
			/>
			{@render label()}
		</summary>
		<div class="flex flex-col gap-2 border-t border-border px-3 py-2">
			{@render children()}
		</div>
	</details>
{:else}
	<p class={[card, row]}>
		{#if Icon}<Icon class="flex-none" size={14} strokeWidth={1.75} />{/if}
		{@render label()}
	</p>
{/if}
